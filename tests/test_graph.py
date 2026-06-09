"""Unit tests for the derived `Graph` view over a loaded index (issue #033)."""
import unittest
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestGraph(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.cfg = self.t.DEFAULT_CONFIG

    def issue(self, iid, status="backlog", parent=None, depends=None,
              priority="medium", kind="task"):
        return self.t.Issue(id=iid, slug=f"i{iid}", title=f"Item {iid}",
                            kind=kind, status=status, priority=priority,
                            parent=parent, depends_on=list(depends or []))

    def graph(self, *issues):
        return self.t.Graph(self.cfg, list(issues))

    # --- maps & lookup ---------------------------------------------------- #

    def test_by_id_indexes_every_row(self):
        g = self.graph(self.issue(1), self.issue(2))
        self.assertEqual(set(g.by_id), {1, 2})
        self.assertIs(g.by_id[1], g.rows[0])

    def test_get_returns_row_or_none(self):
        a = self.issue(1)
        g = self.graph(a)
        self.assertIs(g.get(1), a)
        self.assertIsNone(g.get(99))

    def test_row_returns_issue(self):
        a = self.issue(1)
        self.assertIs(self.graph(a).row(1), a)

    def test_row_dies_on_missing_id(self):
        g = self.graph(self.issue(1))
        with self.assertRaises(SystemExit):
            g.row(99)

    # --- accessors (id-sorted) -------------------------------------------- #

    def test_children_of_returns_id_sorted(self):
        parent = self.issue(1)
        # inserted out of order: child 3 before child 2
        g = self.graph(parent, self.issue(3, parent=1), self.issue(2, parent=1))
        self.assertEqual([c.id for c in g.children_of(parent)], [2, 3])

    def test_children_of_empty_for_leaf(self):
        leaf = self.issue(1)
        self.assertEqual(self.graph(leaf).children_of(leaf), [])

    def test_dependents_of_returns_id_sorted(self):
        dep = self.issue(1)
        # 5 and 3 both depend on 1; inserted 5 before 3
        g = self.graph(dep, self.issue(5, depends=[1]), self.issue(3, depends=[1]))
        self.assertEqual([r.id for r in g.dependents_of(dep)], [3, 5])

    def test_requires_of_returns_existing_deps_id_sorted(self):
        # node depends on 3, 2, and a missing 99 — sorted, missing dropped
        node = self.issue(1, depends=[3, 2, 99])
        g = self.graph(node, self.issue(2), self.issue(3))
        self.assertEqual([r.id for r in g.requires_of(node)], [2, 3])

    # --- predicates ------------------------------------------------------- #

    def test_is_terminal(self):
        g = self.graph(self.issue(1, status="done"), self.issue(2, status="ongoing"))
        self.assertTrue(g.is_terminal(g.row(1)))
        self.assertFalse(g.is_terminal(g.row(2)))

    def test_is_leaf(self):
        parent = self.issue(1)
        child = self.issue(2, parent=1)
        g = self.graph(parent, child)
        self.assertFalse(g.is_leaf(parent))
        self.assertTrue(g.is_leaf(child))

    def test_is_blocked_true_for_open_dependency(self):
        g = self.graph(self.issue(1, status="ongoing"), self.issue(2, depends=[1]))
        self.assertTrue(g.is_blocked(g.row(2)))

    def test_is_blocked_false_when_dependency_terminal(self):
        g = self.graph(self.issue(1, status="done"), self.issue(2, depends=[1]))
        self.assertFalse(g.is_blocked(g.row(2)))

    def test_is_ready_for_unblocked_leaf(self):
        self.assertTrue(self.graph(self.issue(1)).is_ready(self.issue(1)))

    def test_is_ready_false_for_parent(self):
        parent = self.issue(1)
        g = self.graph(parent, self.issue(2, parent=1))
        self.assertFalse(g.is_ready(parent))

    def test_is_ready_false_for_terminal(self):
        done = self.issue(1, status="done")
        self.assertFalse(self.graph(done).is_ready(done))

    def test_is_ready_false_when_blocked(self):
        g = self.graph(self.issue(1, status="ongoing"), self.issue(2, depends=[1]))
        self.assertFalse(g.is_ready(g.row(2)))

    def test_is_ready_true_once_blocker_terminal(self):
        g = self.graph(self.issue(1, status="done"), self.issue(2, depends=[1]))
        self.assertTrue(g.is_ready(g.row(2)))

    # --- dependency cycles ------------------------------------------------ #

    def test_cycles_detects_two_node_cycle(self):
        g = self.graph(self.issue(1, depends=[2]), self.issue(2, depends=[1]))
        cycles = g.cycles()
        self.assertEqual(len(cycles), 1)
        self.assertEqual(set(cycles[0]), {1, 2})

    def test_cycles_detects_self_loop(self):
        g = self.graph(self.issue(1, depends=[1]))
        self.assertEqual(g.cycles(), [[1]])

    def test_cycles_empty_when_acyclic(self):
        g = self.graph(self.issue(1), self.issue(2, depends=[1]))
        self.assertEqual(g.cycles(), [])

    def test_would_cycle_true_for_self_edge(self):
        self.assertTrue(self.graph(self.issue(1)).would_cycle(1, 1))

    def test_would_cycle_true_when_it_closes_a_loop(self):
        # 2 already depends on 1; adding 1 -> 2 would close the loop
        g = self.graph(self.issue(1), self.issue(2, depends=[1]))
        self.assertTrue(g.would_cycle(1, 2))

    def test_would_cycle_false_for_safe_edge(self):
        g = self.graph(self.issue(1), self.issue(2), self.issue(3, depends=[1]))
        self.assertFalse(g.would_cycle(2, 1))     # 2 -> 1 introduces no cycle

    # --- loader ----------------------------------------------------------- #

    def test_load_graph_parallels_load_index(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.t.cmd_new(ns(dir=str(d), title="A", priority="high", kind=None,
                              parent=None, points=None, depends=None, spec=None,
                              slug=None))
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            g = self.t.load_graph(ctx)
            self.assertEqual([r.id for r in g.rows], [1])
            self.assertIn(1, g.by_id)


if __name__ == "__main__":
    unittest.main()
