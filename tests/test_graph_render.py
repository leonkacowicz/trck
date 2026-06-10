"""Unit + integration tests for the `deps --graph` lazygit-style DAG renderer.

Pure-function tests drive the `Graph` directly (mirroring test_graph.py); the
command tests drive `cmd_deps` with `graph=True` (mirroring test_read.py).
"""
import io
import re
import unittest
from contextlib import redirect_stdout
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
        comps = self.t.graph_components(g, [1, 2, 10, 11])
        self.assertEqual(comps, [[1, 2], [10, 11]])

    def test_components_merge_a_diamond(self):
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(3, depends=[1]), self.issue(4, depends=[2, 3]))
        self.assertEqual(self.t.graph_components(g, [1, 2, 3, 4]), [[1, 2, 3, 4]])

    def test_components_ordered_by_smallest_member(self):
        g = self.graph(self.issue(5), self.issue(6, depends=[5]),
                       self.issue(1), self.issue(2, depends=[1]))
        self.assertEqual(self.t.graph_components(g, [5, 6, 1, 2]), [[1, 2], [5, 6]])

    # --- rendering: canonical shapes -------------------------------------- #

    def test_chain_renders_as_stacked_bullets(self):
        # 1 <- 2 <- 3 : a single lane, prerequisites first
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(3, depends=[2]))
        rows = self.t.render_graph(g, [1, 2, 3])
        self.assertEqual(self.order(rows), [1, 2, 3])
        self.assertEqual(self.gutters(rows), ["●", "●", "●"])

    def test_fork_shows_a_branch(self):
        # 1 unblocks both 2 and 3
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(3, depends=[1]))
        rows = self.t.render_graph(g, [1, 2, 3])
        self.assertEqual(self.order(rows), [1, 2, 3])
        self.assertEqual(self.gutters(rows), ["●─╮", "● │", "  ●"])

    def test_diamond_forks_then_merges(self):
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(3, depends=[1]), self.issue(4, depends=[2, 3]))
        rows = self.t.render_graph(g, [1, 2, 3, 4])
        self.assertEqual(self.order(rows), [1, 2, 3, 4])
        self.assertEqual(self.gutters(rows), ["●─╮", "● │", "│ ●", "●─╯"])

    def test_order_is_prerequisites_first(self):
        # every requirement must be rendered above the issue that needs it
        g = self.graph(self.issue(1, depends=[2]), self.issue(2, depends=[3]),
                       self.issue(3))
        order = self.order(self.t.render_graph(g, [1, 2, 3]))
        for r in g.rows:
            for dep in r.depends_on:
                self.assertLess(order.index(dep), order.index(r.id))

    def test_separates_components_with_a_blank_row(self):
        g = self.graph(self.issue(1), self.issue(2, depends=[1]),
                       self.issue(10), self.issue(11, depends=[10]))
        rows = self.t.render_graph(g, [1, 2, 10, 11])
        self.assertIn(None, rows)                       # a separator exists
        self.assertEqual(self.order(rows), [1, 2, 10, 11])
        # exactly one separator, sitting between the two blocks
        self.assertEqual(rows.count(None), 1)
        self.assertIsNone(rows[2])

    def test_single_component_has_no_separator(self):
        g = self.graph(self.issue(1), self.issue(2, depends=[1]))
        rows = self.t.render_graph(g, [1, 2])
        self.assertNotIn(None, rows)

    # --- command: deps --graph -------------------------------------------- #

    def seed(self, d, title="Item", depends=None):
        self.t.cmd_new(ns(dir=str(d), title=title, priority="high", kind=None,
                          parent=None, points=None, depends=depends, spec=None,
                          slug=None))

    def deps_graph(self, d, issue_id=None):
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_deps(ns(dir=str(d), id=issue_id,
                               requires=False, blocks=False, graph=True))
        return buf.getvalue()

    def test_deps_graph_renders_whole_dag_without_an_id(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Base")                        # 1
            self.seed(d, "Mid", depends="1")            # 2
            out = self.deps_graph(d)                    # no id
            self.assertIn("●", out)
            self.assertIn("#001", out)
            self.assertIn("#002", out)

    def test_deps_graph_scopes_to_the_component_of_a_given_id(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A-base")                      # 1
            self.seed(d, "A-top", depends="1")          # 2  (component A)
            self.seed(d, "B-base")                      # 3
            self.seed(d, "B-top", depends="3")          # 4  (component B)
            out = self.deps_graph(d, 1)                 # ask for component A
            self.assertIn("#001", out)
            self.assertIn("#002", out)
            self.assertNotIn("#003", out)               # component B excluded
            self.assertNotIn("#004", out)

    def test_deps_graph_reports_isolated_issue(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Lonely")                      # 1, no deps either way
            out = self.deps_graph(d, 1)
            self.assertIn("#001", out)
            self.assertIn("no dependencies", out.lower())

    def test_deps_without_id_or_graph_is_an_error(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Solo")
            with self.assertRaises(SystemExit):
                with redirect_stdout(io.StringIO()):
                    self.t.cmd_deps(ns(dir=str(d), id=None,
                                       requires=False, blocks=False, graph=False))

    def test_deps_graph_gutter_is_plain_when_color_is_off(self):
        # captured (non-tty) output must carry no ANSI escapes
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Base")                        # 1
            self.seed(d, "Top", depends="1")            # 2
            out = self.deps_graph(d)
            self.assertNotIn("\033[", out)


if __name__ == "__main__":
    unittest.main()
