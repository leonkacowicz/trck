"""Unit + integration tests for the `deps` lazygit-style DAG renderer (the default,
and only, mode of `deps`).

Pure-function tests drive the `Graph` directly (mirroring test_graph.py); the
command tests drive `cmd_deps` (mirroring test_read.py).
"""
import io
import re
import unittest
from contextlib import redirect_stderr, redirect_stdout
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestGraphRender(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.cfg = self.t.DEFAULT_CONFIG

    def issue(self, iid, status="backlog", depends=None):
        return self.t.Issue(id=iid, slug=f"i{iid}", title=f"Item {iid}",
                            kind="task", status=status, priority="medium",
                            parent=None, depends_on=list(depends or []))

    def graph(self, *issues):
        return self.t.Graph(self.cfg, list(issues))

    def gutters(self, rows):
        return [r[1] for r in rows if r is not None]

    def order(self, rows):
        return [r[0] for r in rows if r is not None]

    # --- weakly-connected components -------------------------------------- #

    def test_components_split_disjoint_chains(self):
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(10), self.issue(11, depends=[10]))
        comps = self.t.graph_components(g, ["1", "2", "10", "11"])
        self.assertEqual(comps, [["1", "2"], ["10", "11"]])

    def test_components_merge_a_diamond(self):
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(3, depends=[1]), self.issue(4, depends=[2, 3]))
        self.assertEqual(self.t.graph_components(g, ["1", "2", "3", "4"]), [["1", "2", "3", "4"]])

    def test_components_ordered_by_smallest_member(self):
        g = self.graph(self.issue(5), self.issue(6, depends=[5]),
                       self.issue(1), self.issue(2, depends=[1]))
        self.assertEqual(self.t.graph_components(g, ["5", "6", "1", "2"]), [["1", "2"], ["5", "6"]])

    # --- directed dependency line (focal scoping) ------------------------- #

    def test_dependency_line_excludes_cousins(self):
        # A blocks B, A blocks C, B blocks D  =>  2,3 depend on 1; 4 depends on 2.
        # B's line is {A, B, D}; C is a cousin (shares only the prerequisite A).
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(3, depends=[1]), self.issue(4, depends=[2]))
        self.assertEqual(g.dependency_line(g.row("2")), {"1", "2", "4"})

    def test_dependency_line_is_transitive_both_directions(self):
        # chain 1 <- 2 <- 3 <- 4 <- 5, focus on the middle node
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(3, depends=[2]), self.issue(4, depends=[3]),
                       self.issue(5, depends=[4]))
        self.assertEqual(g.dependency_line(g.row("3")), {"1", "2", "3", "4", "5"})

    def test_dependency_line_up_only_is_the_prerequisite_cone(self):
        # chain 1 <- 2 <- 3; from 2, up scopes to {1, 2} (drops dependent 3)
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(3, depends=[2]))
        self.assertEqual(g.dependency_line(g.row("2"), down=False), {"1", "2"})

    def test_dependency_line_down_only_is_the_dependent_cone(self):
        # chain 1 <- 2 <- 3; from 2, down scopes to {2, 3} (drops prerequisite 1)
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(3, depends=[2]))
        self.assertEqual(g.dependency_line(g.row("2"), up=False), {"2", "3"})

    def test_dependency_line_of_isolated_node_is_just_itself(self):
        g = self.graph(self.issue(1))
        self.assertEqual(g.dependency_line(g.row("1")), {"1"})

    # --- rendering: canonical shapes -------------------------------------- #

    def test_chain_renders_as_stacked_bullets(self):
        # 1 <- 2 <- 3 : a single lane, prerequisites first
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(3, depends=[2]))
        rows = self.t.render_graph(g, ["1", "2", "3"])
        self.assertEqual(self.order(rows), ["1", "2", "3"])
        self.assertEqual(self.gutters(rows), ["●", "●", "●"])

    def test_fork_shows_a_branch(self):
        # 1 unblocks both 2 and 3
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(3, depends=[1]))
        rows = self.t.render_graph(g, ["1", "2", "3"])
        self.assertEqual(self.order(rows), ["1", "2", "3"])
        self.assertEqual(self.gutters(rows), ["●─╮", "● │", "  ●"])

    def test_diamond_forks_then_merges(self):
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(3, depends=[1]), self.issue(4, depends=[2, 3]))
        rows = self.t.render_graph(g, ["1", "2", "3", "4"])
        self.assertEqual(self.order(rows), ["1", "2", "3", "4"])
        self.assertEqual(self.gutters(rows), ["●─╮", "● │", "│ ●", "●─╯"])

    def test_reopened_lane_hugs_the_node_not_the_leftmost_gap(self):
        # 1 forks to 2,3,4 (cols 0,1,2); 2 and 3 finish, freeing cols 0 and 1;
        # then 4 (sitting at col 2) forks again to 5,6. The second new lane must
        # reuse the gap NEAREST the node (col 1), not swing out to the leftmost
        # free column (col 0) — same lane count, but a shorter bridge / no crossing.
        g = self.graph(self.issue(1),
                       self.issue(2, depends=[1]), self.issue(3, depends=[1]),
                       self.issue(4, depends=[1]),
                       self.issue(5, depends=[4]), self.issue(6, depends=[4]))
        rows = self.t.render_graph(g, ["1", "2", "3", "4", "5", "6"])
        self.assertEqual(self.order(rows), ["1", "2", "3", "4", "5", "6"])
        self.assertEqual(self.gutters(rows),
                         ["●─┬─╮", "● │ │", "  ● │", "  ╭─●", "  │ ●", "  ●"])

    def test_nearest_gap_reuse_does_not_widen_the_graph(self):
        # The nearest-gap choice must never use more columns than the optimal
        # (leftmost-free) colouring would: width stays at the max overlap (3 lanes).
        g = self.graph(self.issue(1),
                       self.issue(2, depends=[1]), self.issue(3, depends=[1]),
                       self.issue(4, depends=[1]),
                       self.issue(5, depends=[4]), self.issue(6, depends=[4]))
        rows = self.t.render_graph(g, ["1", "2", "3", "4", "5", "6"])
        # 3 lanes -> at most 3 glyph cells + 2 connectors = width 5
        self.assertEqual(max(len(g) for g in self.gutters(rows)), 5)

    def test_tie_break_finishes_a_branch_before_starting_the_next(self):
        # 1 forks to two independent chains: 1->2->4 and 1->3->5. Ids are laid
        # out so id-priority would interleave them (1,2,3,4,5 = R,A1,B1,A2,B2),
        # zig-zagging the bullets between columns. The DFS/locality tie-break
        # instead finishes one chain fully before the other (1,2,4,3,5), so each
        # chain's bullets stay in a single column — fewer crossings, shorter edges.
        g = self.graph(self.issue(1),
                       self.issue(2, depends=[1]), self.issue(3, depends=[1]),
                       self.issue(4, depends=[2]), self.issue(5, depends=[3]))
        rows = self.t.render_graph(g, ["1", "2", "3", "4", "5"])
        self.assertEqual(self.order(rows), ["1", "2", "4", "3", "5"])
        self.assertEqual(self.gutters(rows),
                         ["●─╮", "● │", "● │", "  ●", "  ●"])

    def test_tie_break_is_deterministic_by_id_within_a_branch(self):
        # Siblings unblocked together are still visited in ascending id order,
        # so the layout is fully deterministic (no reliance on dict/set order).
        g = self.graph(self.issue(1),
                       self.issue(2, depends=[1]), self.issue(3, depends=[1]))
        order = self.order(self.t.render_graph(g, ["1", "2", "3"]))
        self.assertEqual(order, ["1", "2", "3"])

    def test_order_is_prerequisites_first(self):
        # every requirement must be rendered above the issue that needs it
        g = self.graph(self.issue(1, depends=[2]), self.issue(2, depends=[3]),
                       self.issue(3))
        order = self.order(self.t.render_graph(g, ["1", "2", "3"]))
        for r in g.rows:
            for dep in r.depends_on:
                self.assertLess(order.index(dep), order.index(r.id))

    def test_separates_components_with_a_blank_row(self):
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(10), self.issue(11, depends=[10]))
        rows = self.t.render_graph(g, ["1", "2", "10", "11"])
        self.assertIn(None, rows)                       # a separator exists
        self.assertEqual(self.order(rows), ["1", "2", "10", "11"])
        # exactly one separator, sitting between the two blocks
        self.assertEqual(rows.count(None), 1)
        self.assertIsNone(rows[2])

    def test_single_component_has_no_separator(self):
        g = self.graph(self.issue(1), self.issue(2, depends=[1]))
        rows = self.t.render_graph(g, ["1", "2"])
        self.assertNotIn(None, rows)

    # --- command: deps ---------------------------------------------------- #

    def seed(self, d, title="Item", depends=None):
        from pathlib import Path
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(ns(dir=str(d), title=title, priority="high", kind=None,
                              parent=None, points=None, depends=depends, spec=None,
                              slug=None))
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def done(self, d, issue_id):
        with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
            self.t.cmd_mv(ns(dir=str(d), id=issue_id, status="done", resolution=None))

    def deps_graph(self, d, issue_id=None, full=False, omit_done=False,
                   include_done_chains=False):
        buf = io.StringIO()
        # Ensure id is passed as a string (or None) since cmd_deps uses it as a
        # key against g.by_id which holds string ids after Task 1.1 coercion.
        sid = str(issue_id) if issue_id is not None else None
        with redirect_stdout(buf):
            self.t.cmd_deps(ns(dir=str(d), id=sid, full=full,
                               requires=False, blocks=False, graph=True,
                               omit_done=omit_done,
                               include_done_chains=include_done_chains))
        return buf.getvalue()

    def test_deps_graph_renders_whole_dag_without_an_id(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Base")
            id2 = self.seed(d, "Mid", depends=id1)
            out = self.deps_graph(d)                    # no id
            self.assertIn("●", out)
            self.assertIn(f"#{id1}", out)
            self.assertIn(f"#{id2}", out)

    def test_deps_graph_scopes_to_the_component_of_a_given_id(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "A-base")
            id2 = self.seed(d, "A-top", depends=id1)   # component A
            id3 = self.seed(d, "B-base")
            id4 = self.seed(d, "B-top", depends=id3)   # component B
            out = self.deps_graph(d, id1)               # ask for component A
            self.assertIn(f"#{id1}", out)
            self.assertIn(f"#{id2}", out)
            self.assertNotIn(f"#{id3}", out)            # component B excluded
            self.assertNotIn(f"#{id4}", out)

    def test_deps_graph_excludes_cousins_by_default(self):
        # A blocks B, A blocks C, B blocks D: B's graph is A, B, D — not C.
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "A")
            id2 = self.seed(d, "B", depends=id1)
            id3 = self.seed(d, "C", depends=id1)        # cousin of B
            id4 = self.seed(d, "D", depends=id2)
            out = self.deps_graph(d, id2)               # focus on B
            self.assertIn(f"#{id1}", out)               # ancestor A
            self.assertIn(f"#{id2}", out)               # B itself
            self.assertIn(f"#{id4}", out)               # descendant D
            self.assertNotIn(f"#{id3}", out)            # cousin C excluded

    def test_deps_graph_full_includes_the_whole_component(self):
        # --full restores the weakly-connected-component view (cousin included).
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "A")
            id2 = self.seed(d, "B", depends=id1)
            id3 = self.seed(d, "C", depends=id1)        # cousin of B
            id4 = self.seed(d, "D", depends=id2)
            out = self.deps_graph(d, id2, full=True)    # focus on B, whole cluster
            self.assertIn(f"#{id1}", out)
            self.assertIn(f"#{id2}", out)
            self.assertIn(f"#{id3}", out)               # cousin now present
            self.assertIn(f"#{id4}", out)

    def test_deps_graph_reports_isolated_issue(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Lonely")                # no deps either way
            out = self.deps_graph(d, id1)
            self.assertIn(f"#{id1}", out)
            self.assertIn("no dependencies", out.lower())

    def test_deps_without_id_renders_the_whole_graph_by_default(self):
        # graph is the default mode now: no --graph flag, no id needed.
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Base")
            id2 = self.seed(d, "Top", depends=id1)
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_deps(ns(dir=str(d), id=None, requires=False,
                                   blocks=False, full=False))
            out = buf.getvalue()
            self.assertIn("●", out)
            self.assertIn(f"#{id1}", out)
            self.assertIn(f"#{id2}", out)

    def test_deps_graph_gutter_is_plain_when_color_is_off(self):
        # captured (non-tty) output must carry no ANSI escapes
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Base")
            self.seed(d, "Top", depends=id1)
            out = self.deps_graph(d)
            self.assertNotIn("\033[", out)

    # --- done filtering ---------------------------------------------------- #

    def test_deps_whole_graph_hides_fully_done_chains_by_default(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            done_base = self.seed(d, "Done base")
            done_top = self.seed(d, "Done top", depends=done_base)
            live_base = self.seed(d, "Live base")
            live_top = self.seed(d, "Live top", depends=live_base)
            self.done(d, done_base)
            self.done(d, done_top)

            out = self.deps_graph(d)

            self.assertNotIn(f"#{done_base}", out)
            self.assertNotIn(f"#{done_top}", out)
            self.assertIn(f"#{live_base}", out)
            self.assertIn(f"#{live_top}", out)

    def test_deps_include_done_chains_restores_fully_done_components(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            done_base = self.seed(d, "Done base")
            done_top = self.seed(d, "Done top", depends=done_base)
            self.done(d, done_base)
            self.done(d, done_top)

            out = self.deps_graph(d, include_done_chains=True)

            self.assertIn(f"#{done_base}", out)
            self.assertIn(f"#{done_top}", out)

    def test_deps_keeps_done_nodes_in_mixed_chains_by_default(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            done_base = self.seed(d, "Done base")
            live_top = self.seed(d, "Live top", depends=done_base)
            self.done(d, done_base)

            out = self.deps_graph(d)

            self.assertIn(f"#{done_base}", out)
            self.assertIn(f"#{live_top}", out)

    def test_deps_omit_done_drops_done_nodes_without_bridging_edges(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            base = self.seed(d, "Base")
            mid = self.seed(d, "Mid", depends=base)
            top = self.seed(d, "Top", depends=mid)
            self.done(d, mid)

            out = self.deps_graph(d, omit_done=True)

            self.assertIn(f"#{base}", out)
            self.assertNotIn(f"#{mid}", out)
            self.assertIn(f"#{top}", out)
            self.assertEqual(out.count("●"), 2)
            self.assertIn("\n\n", out)  # recomputed as two singleton components

    def test_deps_omit_done_hides_even_included_done_chains(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            done_base = self.seed(d, "Done base")
            done_top = self.seed(d, "Done top", depends=done_base)
            self.done(d, done_base)
            self.done(d, done_top)

            out = self.deps_graph(d, omit_done=True, include_done_chains=True)

            self.assertEqual(out, "")

    def test_deps_single_id_keeps_done_chain_by_default(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            done_base = self.seed(d, "Done base")
            done_top = self.seed(d, "Done top", depends=done_base)
            self.done(d, done_base)
            self.done(d, done_top)

            out = self.deps_graph(d, done_base)

            self.assertIn(f"#{done_base}", out)
            self.assertIn(f"#{done_top}", out)

    def test_deps_done_filter_uses_configured_terminal_role(self):
        cfg = dict(self.t.DEFAULT_CONFIG)
        cfg["statuses"] = [
            {"name": "todo", "role": "initial"},
            {"name": "doing", "role": "active"},
            {"name": "shipped", "role": "terminal"},
        ]
        cfg["aliases"] = {"start": "doing", "done": "shipped"}
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, cfg)
            done_base = self.seed(d, "Done base")
            done_top = self.seed(d, "Done top", depends=done_base)
            with redirect_stdout(io.StringIO()):
                self.t.cmd_mv(ns(dir=str(d), id=done_base, status="shipped", resolution=None))
                self.t.cmd_mv(ns(dir=str(d), id=done_top, status="shipped", resolution=None))

            out = self.deps_graph(d)

            self.assertNotIn(f"#{done_base}", out)
            self.assertNotIn(f"#{done_top}", out)

    # --- focal-row highlight (deps NNN) ----------------------------------- #

    def row_with(self, out, needle):
        return next(ln for ln in out.splitlines() if needle in ln)

    def test_deps_marks_only_the_focal_row_with_a_caret(self):
        # color off: the ▸ marker (color-independent) sits on the focal row only
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Base")
            id2 = self.seed(d, "Top", depends=id1)
            out = self.deps_graph(d, id2)               # focus on id2
            self.assertTrue(self.row_with(out, f"#{id2}").startswith("▸"))
            self.assertFalse(self.row_with(out, f"#{id1}").startswith("▸"))
            # context rows keep their columns aligned under the marker gutter
            self.assertTrue(self.row_with(out, f"#{id1}").startswith("  "))

    def test_deps_whole_graph_has_no_focal_marker(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Base")
            self.seed(d, "Top", depends=id1)
            out = self.deps_graph(d)                    # no id -> no focal row
            self.assertNotIn("▸", out)

    def test_deps_focal_row_marks_caret_bolds_title_and_highlights_id_prefix(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Base")
            id2 = self.seed(d, "Top", depends=id1)
            self.t._use_color = lambda: True
            out = self.deps_graph(d, id2)
            self.assertIn(self.t.paint("▸", "bold"), out)          # focal marked by caret
            self.assertIn(self.t.paint("Top", "bold"), out)         # focal title bold
            # focal id is NOT wholly bold; it gets the shortest-unique-prefix highlight
            self.assertNotIn(self.t.paint(f"#{id2}", "bold"), out)
            L = self.t.unique_prefix_lens([id1, id2])[id2]
            self.assertIn(self.t.paint(id2[:L], "bold"), out)       # unique prefix bold
            if id2[L:]:
                self.assertIn(self.t.paint(id2[L:], "dim"), out)    # remainder dimmed

    # --- label dimming (node_label, the shared `deps` row renderer) -------- #

    def node_label_out(self, d, **over):
        ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
        base = dict(id=1, slug="i1", title="Alpha", kind="task",
                    status="backlog", priority="high")
        base.update(over)
        return self.t.node_label(ctx, self.t.Issue(**base))

    def test_node_label_dims_the_label_tag_like_list_and_tree(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.t._use_color = lambda: True
            out = self.node_label_out(d, labels=["combat"])
            # the label bracket is wrapped in dim, exactly as print_rows renders it
            self.assertIn(self.t.paint(" [combat]", "dim"), out)
            # the title is not swallowed into the dim run
            self.assertIn("Alpha", out)
            self.assertNotIn(self.t.paint("Alpha", "dim"), out)

    def test_node_label_has_no_bracket_without_labels(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.t._use_color = lambda: True
            out = self.node_label_out(d, labels=[])
            plain = re.sub("\033\\[[0-9;]*m", "", out)
            self.assertNotIn("[", plain)            # no label bracket, no stray dim


if __name__ == "__main__":
    unittest.main()
