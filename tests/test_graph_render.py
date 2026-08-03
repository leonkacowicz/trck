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

    def issue(self, iid, status="backlog", depends=None, parent=None):
        return self.t.Issue(id=iid, slug=f"i{iid}", title=f"Item {iid}",
                            kind="task", status=status, priority="medium",
                            parent=parent, depends_on=list(depends or []))

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

    def test_a_lone_blocker_moves_down_to_shorten_its_lane(self):
        # 1 blocks only 4; 2 -> 3 -> 4 is a chain. Prerequisites-first alone puts 1 first
        # (lowest id among the roots), so its lane hangs open beside the whole chain.
        # Nothing forces that: 1 need only precede 4, and sliding it down to just above
        # 4 costs the chain one row and saves 1 two, for a shorter gutter overall.
        g = self.graph(self.issue(1), self.issue(2), self.issue(3, depends=[2]),
                       self.issue(4, depends=[1, 3]))
        self.assertEqual(self.order(self.t.render_graph(g, ["1", "2", "3", "4"])),
                         ["2", "3", "1", "4"])

    def test_shortening_never_breaks_prerequisites_first(self):
        g = self.graph(self.issue(1), self.issue(2), self.issue(3, depends=[2]),
                       self.issue(4, depends=[1, 3]), self.issue(5, depends=[1]),
                       self.issue(6, depends=[4, 5]))
        ids = ["1", "2", "3", "4", "5", "6"]
        order = self.order(self.t.render_graph(g, ids))
        self.assertCountEqual(order, ids)
        for r in g.rows:
            for dep in r.depends_on:
                self.assertLess(order.index(dep), order.index(r.id))

    def test_shortening_is_independent_of_the_input_id_order(self):
        """Same graph, ids handed over in a different order, byte-identical rows.

        The search runs off sets and dict lookups in places, so the guard that matters is
        not "no RNG" but that nothing downstream of a set's iteration order reaches the
        output. A canonical start order plus order-free accumulation is what buys that."""
        g = self.graph(self.issue(1), self.issue(2), self.issue(3, depends=[2]),
                       self.issue(4, depends=[1, 3]), self.issue(5, depends=[1]),
                       self.issue(6, depends=[4, 5]))
        ids = ["1", "2", "3", "4", "5", "6"]
        first = self.t.render_graph(g, ids)
        for shuffled in (list(reversed(ids)), ["4", "1", "6", "3", "5", "2"]):
            self.assertEqual(self.t.render_graph(g, shuffled), first)

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

    def test_deps_done_filter_reads_terminality_not_a_hard_coded_name(self):
        """The filter asks `is_terminal`, so it hides a fully-settled chain because those
        issues are finished — not because their status happens to be spelled a given way."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            done_base = self.seed(d, "Done base")
            done_top = self.seed(d, "Done top", depends=done_base)
            with redirect_stdout(io.StringIO()):
                self.t.cmd_mv(ns(dir=str(d), id=done_base, status="done", resolution=None))
                self.t.cmd_mv(ns(dir=str(d), id=done_top, status="done", resolution=None))

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


class TestContainmentEdges(unittest.TestCase):
    """`deps` draws an inferred `parent -> child` edge for every parent/child pair:
    a parent is done exactly when its children are, which *is* a dependency. Without
    them the graph cannot answer "what is needed to complete this epic". Inferred
    edges are display-only — nothing here is ever written back to the index."""

    def setUp(self):
        self.t = load_trck()
        self.cfg = self.t.DEFAULT_CONFIG

    def issue(self, iid, status="backlog", depends=None, parent=None):
        return self.t.Issue(id=iid, slug=f"i{iid}", title=f"Item {iid}",
                            kind="task", status=status, priority="medium",
                            parent=parent, depends_on=list(depends or []))

    def graph(self, *issues):
        return self.t.Graph(self.cfg, list(issues))

    def order(self, rows):
        return [r[0] for r in rows if r is not None]

    # --- the edge accessor ------------------------------------------------ #

    def test_a_parent_depends_on_each_of_its_children(self):
        g = self.graph(self.issue(1), self.issue(2, parent="1"), self.issue(3, parent="1"))
        self.assertEqual([(b.id, kind) for b, kind in g.drawn_deps_of(g.row("1"))],
                         [("2", "child"), ("3", "child")])

    def test_authored_edges_keep_their_kind_alongside_children(self):
        g = self.graph(self.issue(1, depends=[9]), self.issue(2, parent="1"),
                       self.issue(9))
        self.assertEqual([(b.id, kind) for b, kind in g.drawn_deps_of(g.row("1"))],
                         [("9", "dep"), ("2", "child")])

    def test_hier_false_is_the_authored_graph_untouched(self):
        g = self.graph(self.issue(1, depends=[9]), self.issue(2, parent="1"),
                       self.issue(9))
        self.assertEqual([(b.id, kind) for b, kind in g.drawn_deps_of(g.row("1"), hier=False)],
                         [("9", "dep")])

    def test_a_child_does_not_gain_an_edge_back_to_its_parent(self):
        # containment points parent -> child only; the reverse would be a cycle
        g = self.graph(self.issue(1), self.issue(2, parent="1"))
        self.assertEqual(g.drawn_deps_of(g.row("2")), [])

    def test_a_missing_parent_reference_is_ignored(self):
        g = self.graph(self.issue(2, parent="nope"))
        self.assertEqual(g.drawn_deps_of(g.row("2")), [])

    # --- components / ordering -------------------------------------------- #

    def test_a_family_is_one_component(self):
        g = self.graph(self.issue(1), self.issue(2, parent="1"), self.issue(3, parent="1"))
        self.assertEqual(self.t.graph_components(g, ["1", "2", "3"]), [["1", "2", "3"]])

    def test_children_render_above_their_parent(self):
        # a child blocks its parent, and the renderer puts blockers on top — so an
        # epic sits *below* the work it contains, the last thing to complete.
        g = self.graph(self.issue(1), self.issue(2, parent="1"), self.issue(3, parent="1"))
        self.assertEqual(self.order(self.t.render_graph(g, ["1", "2", "3"])), ["2", "3", "1"])

    def test_a_childs_own_prerequisite_precedes_the_whole_family(self):
        # 9 blocks child 2, child 2 blocks parent 1
        g = self.graph(self.issue(1), self.issue(2, parent="1", depends=[9]), self.issue(9))
        self.assertEqual(self.order(self.t.render_graph(g, ["1", "2", "9"])), ["9", "2", "1"])

    # --- dependency line (scoped view) ------------------------------------ #

    def test_a_parents_line_reaches_its_whole_subtree(self):
        g = self.graph(self.issue(1), self.issue(2, parent="1"),
                       self.issue(3, parent="2"))
        self.assertEqual(g.dependency_line(g.row("1"), down=False), {"1", "2", "3"})

    def test_a_childs_line_reaches_up_to_its_parent(self):
        g = self.graph(self.issue(1), self.issue(2, parent="1"))
        self.assertEqual(g.dependency_line(g.row("2"), up=False), {"1", "2"})

    def test_siblings_stay_cousins(self):
        # 2 and 3 share only the parent that contains them; the cone must not cross
        g = self.graph(self.issue(1), self.issue(2, parent="1"), self.issue(3, parent="1"))
        self.assertEqual(g.dependency_line(g.row("2"), up=False), {"1", "2"})

    # --- command level ---------------------------------------------------- #

    def seed(self, d, title="Item", depends=None, parent=None):
        from pathlib import Path
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(ns(dir=str(d), title=title, priority="high", kind=None,
                              parent=parent, points=None, depends=depends, spec=None,
                              slug=None))
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def deps_graph(self, d, issue_id=None, **over):
        args = dict(dir=str(d), id=str(issue_id) if issue_id is not None else None,
                    full=False, requires=False, blocks=False, omit_done=False,
                    include_done_chains=False)
        args.update(over)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_deps(ns(**args))
        return buf.getvalue()

    def test_deps_of_an_epic_lists_the_work_it_contains(self):
        # the headline case: before containment edges this printed "(no dependencies)"
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            epic = self.seed(d, "Epic")
            kid1 = self.seed(d, "Kid one", parent=epic)
            kid2 = self.seed(d, "Kid two", parent=epic)
            out = self.deps_graph(d, epic)
            self.assertNotIn("no dependencies", out.lower())
            self.assertIn(f"#{kid1}", out)
            self.assertIn(f"#{kid2}", out)

    def test_deps_of_an_epic_reaches_a_childs_external_blocker(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker")
            epic = self.seed(d, "Epic")
            self.seed(d, "Kid", parent=epic, depends=blocker)
            out = self.deps_graph(d, epic, requires=True)
            self.assertIn(f"#{blocker}", out)

    def test_bare_deps_drops_a_family_with_no_authored_edge(self):
        # pure hierarchy is what `list` is for; admitting it turns `deps` into the
        # forest. A family only appears once something in it is actually ordered.
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            lone_epic = self.seed(d, "Lone epic")
            lone_kid = self.seed(d, "Lone kid", parent=lone_epic)
            base = self.seed(d, "Base")
            top = self.seed(d, "Top", depends=base)
            out = self.deps_graph(d)
            self.assertIn(f"#{base}", out)
            self.assertIn(f"#{top}", out)
            self.assertNotIn(f"#{lone_epic}", out)
            self.assertNotIn(f"#{lone_kid}", out)

    def test_bare_deps_keeps_a_family_whole_once_it_has_an_authored_edge(self):
        # the reason to drop whole components rather than filter nodes: a partially
        # shown epic would misreport what it needs. `sib` has no edges of its own
        # and must still appear, or the epic's row is a lie.
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker")
            epic = self.seed(d, "Epic")
            kid = self.seed(d, "Kid", parent=epic, depends=blocker)
            sib = self.seed(d, "Sibling", parent=epic)
            out = self.deps_graph(d)
            for iid in (blocker, epic, kid, sib):
                self.assertIn(f"#{iid}", out)

    # --- telling inferred edges apart from authored ones ------------------ #

    def test_a_containment_lane_is_dimmed(self):
        # box-drawing has no dashed corner glyphs, so weight carries the distinction
        self.t._use_color = lambda: True
        self.assertIn(self.t._ANSI["dim"], self.t.paint_lane("│", ("3", "child")))

    def test_an_authored_lane_is_not_dimmed(self):
        self.t._use_color = lambda: True
        self.assertNotIn(self.t._ANSI["dim"], self.t.paint_lane("│", ("3", "dep")))

    def test_both_kinds_keep_the_same_palette_colour_for_one_lane(self):
        # dimming must not change the hue, or a lane stops being traceable
        self.t._use_color = lambda: True
        hue = lambda s: re.findall(r"\033\[(3[0-7]|9[0-6])m", s)
        self.assertEqual(hue(self.t.paint_lane("│", ("3", "dep"))),
                         hue(self.t.paint_lane("│", ("3", "child"))))

    def test_a_bare_id_owner_still_works(self):
        # paint_lane's older single-argument owner form stays supported
        self.t._use_color = lambda: True
        self.assertEqual(self.t.paint_lane("│", "3"), self.t.paint_lane("│", ("3", "dep")))

    def test_containment_lanes_are_dim_in_real_deps_output(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker")
            epic = self.seed(d, "Epic")
            self.seed(d, "Kid", parent=epic, depends=blocker)
            self.t._use_color = lambda: True
            self.assertIn(self.t._ANSI["dim"], self.deps_graph(d, epic))

    def test_deps_never_writes_inferred_edges_to_the_index(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            epic = self.seed(d, "Epic")
            self.seed(d, "Kid", parent=epic)
            before = (d / "index.jsonl").read_text()
            self.deps_graph(d, epic)
            self.assertEqual((d / "index.jsonl").read_text(), before)


class TestTransitiveReduction(unittest.TestCase):
    """The drawn graph omits any edge already implied by a longer path: with A needing
    both B and C, and B needing C, draw A <- B <- C and drop A <- C. On a DAG the
    reduction is unique, so there is nothing arbitrary to choose. Display-only — the
    authored edge stays in the index, and only `dep --remove` deletes it."""

    def setUp(self):
        self.t = load_trck()
        self.cfg = self.t.DEFAULT_CONFIG

    def issue(self, iid, status="backlog", depends=None, parent=None):
        return self.t.Issue(id=iid, slug=f"i{iid}", title=f"Item {iid}",
                            kind="task", status=status, priority="medium",
                            parent=parent, depends_on=list(depends or []))

    def graph(self, *issues):
        return self.t.Graph(self.cfg, list(issues))

    def edges(self, g, ids, **over):
        """The drawn edge set as {source: {target}}, ignoring edge kinds."""
        e = self.t.drawn_edges(g, ids, **over)
        return {i: {t for t, _k in tv} for i, tv in e.items() if tv}

    def gutters(self, rows):
        return [r[1] for r in rows if r is not None]

    # --- the reduction itself --------------------------------------------- #

    def test_drops_an_edge_implied_by_a_two_hop_path(self):
        # A needs B and C; B needs C  =>  A -> C is implied by A -> B -> C
        g = self.graph(self.issue("a", depends=["b", "c"]),
                       self.issue("b", depends=["c"]), self.issue("c"))
        self.assertEqual(self.edges(g, ["a", "b", "c"], reduce=True),
                         {"a": {"b"}, "b": {"c"}})

    def test_keeps_everything_without_reduction(self):
        g = self.graph(self.issue("a", depends=["b", "c"]),
                       self.issue("b", depends=["c"]), self.issue("c"))
        self.assertEqual(self.edges(g, ["a", "b", "c"], reduce=False),
                         {"a": {"b", "c"}, "b": {"c"}})

    def test_a_chain_is_already_reduced(self):
        g = self.graph(self.issue("a", depends=["b"]), self.issue("b", depends=["c"]),
                       self.issue("c"))
        self.assertEqual(self.edges(g, ["a", "b", "c"], reduce=True),
                         {"a": {"b"}, "b": {"c"}})

    def test_a_diamond_keeps_all_four_edges(self):
        # nothing is implied twice over: neither branch reaches the other
        g = self.graph(self.issue("a", depends=["b", "c"]), self.issue("b", depends=["d"]),
                       self.issue("c", depends=["d"]), self.issue("d"))
        self.assertEqual(self.edges(g, ["a", "b", "c", "d"], reduce=True),
                         {"a": {"b", "c"}, "b": {"d"}, "c": {"d"}})

    def test_drops_an_edge_implied_by_a_long_path(self):
        g = self.graph(self.issue("a", depends=["b", "d"]), self.issue("b", depends=["c"]),
                       self.issue("c", depends=["d"]), self.issue("d"))
        self.assertEqual(self.edges(g, ["a", "b", "c", "d"], reduce=True),
                         {"a": {"b"}, "b": {"c"}, "c": {"d"}})

    def test_reduction_is_idempotent(self):
        g = self.graph(self.issue("a", depends=["b", "c"]),
                       self.issue("b", depends=["c"]), self.issue("c"))
        once = self.t.drawn_edges(g, ["a", "b", "c"], reduce=True)
        self.assertEqual(self.t.transitive_reduction(once), once)

    def test_reduction_preserves_reachability(self):
        g = self.graph(self.issue("a", depends=["b", "c"]),
                       self.issue("b", depends=["c"]), self.issue("c"))
        ids = ["a", "b", "c"]

        def reach(e, start):
            seen, stack = set(), [start]
            while stack:
                for t, _k in e.get(stack.pop(), []):
                    if t not in seen:
                        seen.add(t); stack.append(t)
            return seen

        full = self.t.drawn_edges(g, ids, reduce=False)
        cut = self.t.drawn_edges(g, ids, reduce=True)
        for i in ids:
            self.assertEqual(reach(full, i), reach(cut, i))

    # --- containment edges reduce too ------------------------------------- #

    def test_a_parent_points_only_at_the_work_nothing_else_waits_on(self):
        # epic P contains 1 and 2, and 1 needs 2. P -> 2 is implied by P -> 1 -> 2,
        # so the epic points only at 1 — the maximal element of its subtree.
        g = self.graph(self.issue("p"), self.issue("1", parent="p", depends=["2"]),
                       self.issue("2", parent="p"))
        self.assertEqual(self.edges(g, ["p", "1", "2"], reduce=True),
                         {"p": {"1"}, "1": {"2"}})

    def test_a_kept_edge_keeps_its_kind(self):
        g = self.graph(self.issue("p"), self.issue("1", parent="p", depends=["2"]),
                       self.issue("2", parent="p"))
        e = self.t.drawn_edges(g, ["p", "1", "2"], reduce=True)
        self.assertEqual(e["p"], [("1", "child")])
        self.assertEqual(e["1"], [("2", "dep")])

    # --- rendering -------------------------------------------------------- #

    def test_the_implied_edge_costs_no_lane(self):
        # unreduced this needs two lanes out of A; reduced it is a plain chain
        g = self.graph(self.issue("a", depends=["b", "c"]),
                       self.issue("b", depends=["c"]), self.issue("c"))
        rows = self.t.render_graph(g, ["a", "b", "c"])
        self.assertEqual(self.gutters(rows), ["●", "●", "●"])

    def test_unreduced_rendering_still_available(self):
        g = self.graph(self.issue("a", depends=["b", "c"]),
                       self.issue("b", depends=["c"]), self.issue("c"))
        rows = self.t.render_graph(g, ["a", "b", "c"], reduce=False)
        self.assertNotEqual(self.gutters(rows), ["●", "●", "●"])

    # --- the ordering trap ------------------------------------------------- #

    def test_hiding_a_middle_node_must_not_disconnect_its_neighbours(self):
        # A -> B -> C plus an authored A -> C. Reduce *before* dropping the done B
        # and A -> C vanishes with nothing left to imply it — A and C would render
        # as unrelated. Reducing the already-filtered set cannot do that: a path
        # only justifies dropping an edge when the path is itself drawn.
        g = self.graph(self.issue("a", depends=["b", "c"]),
                       self.issue("b", status="done", depends=["c"]), self.issue("c"))
        self.assertEqual(self.edges(g, ["a", "c"], reduce=True), {"a": {"c"}})
        rows = self.t.render_graph(g, ["a", "c"])
        self.assertNotIn(None, rows)             # one component, not two

    def test_reduction_runs_on_the_filtered_set_end_to_end(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            base = self.seed(d, "Base")
            mid = self.seed(d, "Mid", depends=base)
            top = self.seed(d, "Top", depends=f"{mid},{base}")
            self.done(d, mid)
            out = self.deps_graph(d, omit_done=True)
            # Mid is gone, so Top -> Base is the only thing tying them together and
            # must survive: both on screen, no blank line splitting them apart.
            self.assertIn(f"#{base}", out)
            self.assertIn(f"#{top}", out)
            self.assertNotIn(f"#{mid}", out)
            self.assertNotIn("\n\n", out.strip())

    # --- command level ----------------------------------------------------- #

    def seed(self, d, title="Item", depends=None, parent=None):
        from pathlib import Path
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(ns(dir=str(d), title=title, priority="high", kind=None,
                              parent=parent, points=None, depends=depends, spec=None,
                              slug=None))
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def done(self, d, issue_id):
        with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
            self.t.cmd_mv(ns(dir=str(d), id=issue_id, status="done", resolution=None))

    def deps_graph(self, d, issue_id=None, **over):
        args = dict(dir=str(d), id=str(issue_id) if issue_id is not None else None,
                    full=False, requires=False, blocks=False, omit_done=False,
                    include_done_chains=False)
        args.update(over)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_deps(ns(**args))
        return buf.getvalue()

    def test_the_redundant_edge_stays_in_the_index(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            base = self.seed(d, "Base")
            mid = self.seed(d, "Mid", depends=base)
            top = self.seed(d, "Top", depends=f"{mid},{base}")
            before = (d / "index.jsonl").read_text()
            self.deps_graph(d)
            self.assertEqual((d / "index.jsonl").read_text(), before)
            import json
            stored = {r["id"]: r for r in map(json.loads, before.splitlines())}
            # the implied edge is hidden from the graph, not deleted: `dep --remove`
            # stays the only way to drop it
            self.assertEqual(sorted(stored[top]["depends_on"]), sorted([mid, base]))


class TestInheritedEdges(unittest.TestCase):
    """An authored edge is inherited by the author's whole subtree, so a child blocked
    only through its parent must be able to show it. Drawn per-child only where the
    ancestor carrying the edge isn't itself on screen — mirroring the `needs #X (via
    #P)` note — because inheritance is uniform by construction: restating it under
    every child of a visible parent is pure fan-out. `--fanout` restates it anyway."""

    def setUp(self):
        self.t = load_trck()
        self.cfg = self.t.DEFAULT_CONFIG

    def issue(self, iid, status="backlog", depends=None, parent=None):
        return self.t.Issue(id=iid, slug=f"i{iid}", title=f"Item {iid}",
                            kind="task", status=status, priority="medium",
                            parent=parent, depends_on=list(depends or []))

    def graph(self, *issues):
        return self.t.Graph(self.cfg, list(issues))

    def family(self):
        """epic p needs x; children 1 and 2 inherit that need."""
        return self.graph(self.issue("p", depends=["x"]), self.issue("1", parent="p"),
                          self.issue("2", parent="p"), self.issue("x"))

    def edges(self, g, ids, **over):
        e = self.t.drawn_edges(g, ids, **over)
        return {i: {t for t, _k in tv} for i, tv in e.items() if tv}

    # --- the inferred edge ------------------------------------------------- #

    def test_a_child_inherits_its_parents_dependency(self):
        g = self.family()
        self.assertIn(("x", "inherited"),
                      [(b.id, k) for b, k in g.drawn_deps_of(g.row("1"))])

    def test_an_own_edge_outranks_an_inherited_one(self):
        # nearest author wins: a child that authored the edge itself is not inheriting
        g = self.graph(self.issue("p", depends=["x"]), self.issue("1", parent="p", depends=["x"]),
                       self.issue("x"))
        self.assertEqual([(b.id, k) for b, k in g.drawn_deps_of(g.row("1"))], [("x", "dep")])

    def test_inheritance_reaches_a_grandchild(self):
        g = self.graph(self.issue("p", depends=["x"]), self.issue("1", parent="p"),
                       self.issue("2", parent="1"), self.issue("x"))
        self.assertIn(("x", "inherited"),
                      [(b.id, k) for b, k in g.drawn_deps_of(g.row("2"))])

    def test_no_inherited_edge_points_inside_the_childs_own_subtree(self):
        # an authored ancestor/descendant edge is already rejected as a cycle, so an
        # inherited target can never be a node the child contains
        g = self.graph(self.issue("p", depends=["x"]), self.issue("1", parent="p"),
                       self.issue("2", parent="1"), self.issue("x"))
        for r in g.rows:
            own = {n.id for n in g.subtree(r)}
            for b, kind in g.drawn_deps_of(r):
                if kind == "inherited":
                    self.assertNotIn(b.id, own)

    def test_the_cone_of_a_child_reaches_its_inherited_blocker(self):
        g = self.family()
        self.assertIn("x", g.dependency_line(g.row("1"), down=False))

    # --- the on-screen rule (the hoist) ------------------------------------ #

    def test_a_visible_parent_carries_the_edge_alone(self):
        # p is on screen and already says it needs x; restating it under 1 and 2
        # would replace one parent-altitude edge with a fan of n
        g = self.family()
        self.assertEqual(self.edges(g, ["p", "1", "2", "x"]),
                         {"p": {"x", "1", "2"}})

    def test_an_absent_parent_hands_the_edge_down(self):
        # scoped below p: nothing on screen carries the need, so 1 must show it
        g = self.family()
        self.assertEqual(self.edges(g, ["1", "x"]), {"1": {"x"}})

    def test_the_nearest_visible_ancestor_wins(self):
        g = self.graph(self.issue("p", depends=["x"]), self.issue("1", parent="p"),
                       self.issue("2", parent="1"), self.issue("x"))
        # 1 is on screen and carries it, so the grandchild stays quiet
        self.assertEqual(self.edges(g, ["1", "2", "x"]), {"1": {"x", "2"}})

    def test_the_parents_authored_edge_survives_reduction(self):
        # the whole point of suppressing the fan: without it p -> x is implied by
        # p -> 1 -> x and reduction deletes it, demoting an epic-level dependency
        g = self.family()
        self.assertIn("x", self.edges(g, ["p", "1", "2", "x"])["p"])

    # --- --fanout ---------------------------------------------------------- #

    def test_fanout_restates_the_edge_under_every_child(self):
        g = self.family()
        self.assertEqual(self.edges(g, ["p", "1", "2", "x"], fanout=True),
                         {"p": {"1", "2"}, "1": {"x"}, "2": {"x"}})

    def test_fanout_costs_the_parent_its_own_edge(self):
        # p -> x becomes implied by p -> 1 -> x, so reduction drops it. Nothing is
        # lost: p still reaches x through the children, which is the ground truth
        # about which specific work is blocked.
        g = self.family()
        self.assertNotIn("x", self.edges(g, ["p", "1", "2", "x"], fanout=True)["p"])

    def test_fanout_keeps_reachability(self):
        g = self.family()
        for mode in (False, True):
            e = self.t.drawn_edges(g, ["p", "1", "2", "x"], fanout=mode)
            seen, stack = set(), ["p"]
            while stack:
                for t, _k in e.get(stack.pop(), []):
                    if t not in seen:
                        seen.add(t); stack.append(t)
            self.assertEqual(seen, {"1", "2", "x"}, f"fanout={mode}")

    # --- command level ------------------------------------------------------ #

    def seed(self, d, title="Item", depends=None, parent=None):
        from pathlib import Path
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(ns(dir=str(d), title=title, priority="high", kind=None,
                              parent=parent, points=None, depends=depends, spec=None,
                              slug=None))
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def deps_graph(self, d, issue_id=None, **over):
        args = dict(dir=str(d), id=str(issue_id) if issue_id is not None else None,
                    full=False, requires=False, blocks=False, omit_done=False,
                    include_done_chains=False, fanout=False)
        args.update(over)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_deps(ns(**args))
        return buf.getvalue()

    def test_a_child_scoped_to_its_requirements_shows_the_inherited_blocker(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker")
            epic = self.seed(d, "Epic", depends=blocker)
            kid = self.seed(d, "Kid", parent=epic)
            out = self.deps_graph(d, kid, requires=True)
            self.assertIn(f"#{blocker}", out)       # the epic is not in this cone
            self.assertIn(f"#{kid}", out)

    def test_fanout_is_off_by_default(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker")
            epic = self.seed(d, "Epic", depends=blocker)
            self.seed(d, "Kid one", parent=epic)
            self.seed(d, "Kid two", parent=epic)
            self.assertNotEqual(self.deps_graph(d, epic),
                                self.deps_graph(d, epic, fanout=True))

    def test_deps_never_writes_inherited_edges_to_the_index(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker")
            epic = self.seed(d, "Epic", depends=blocker)
            self.seed(d, "Kid", parent=epic)
            before = (d / "index.jsonl").read_text()
            self.deps_graph(d, epic, fanout=True)
            self.assertEqual((d / "index.jsonl").read_text(), before)


if __name__ == "__main__":
    unittest.main()
