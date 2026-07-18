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
        self.assertEqual(set(g.by_id), {"1", "2"})
        self.assertIs(g.by_id["1"], g.rows[0])

    def test_get_returns_row_or_none(self):
        a = self.issue(1)
        g = self.graph(a)
        self.assertIs(g.get("1"), a)
        self.assertIsNone(g.get(99))

    def test_row_returns_issue(self):
        a = self.issue(1)
        self.assertIs(self.graph(a).row("1"), a)

    def test_row_dies_on_missing_id(self):
        g = self.graph(self.issue(1))
        with self.assertRaises(SystemExit):
            g.row(99)

    # --- accessors (id-sorted) -------------------------------------------- #

    def test_children_of_returns_id_sorted(self):
        parent = self.issue(1)
        # inserted out of order: child 3 before child 2
        g = self.graph(parent, self.issue(3, parent=1), self.issue(2, parent=1))
        self.assertEqual([c.id for c in g.children_of(parent)], ["2", "3"])

    def test_children_of_empty_for_leaf(self):
        leaf = self.issue(1)
        self.assertEqual(self.graph(leaf).children_of(leaf), [])

    def test_dependents_of_returns_id_sorted(self):
        dep = self.issue(1)
        # 5 and 3 both depend on 1; inserted 5 before 3
        g = self.graph(dep, self.issue(5, depends=[1]), self.issue(3, depends=[1]))
        self.assertEqual([r.id for r in g.dependents_of(dep)], ["3", "5"])

    def test_requires_of_returns_existing_deps_id_sorted(self):
        # node depends on 3, 2, and a missing 99 — sorted, missing dropped
        node = self.issue(1, depends=[3, 2, 99])
        g = self.graph(node, self.issue(2), self.issue(3))
        self.assertEqual([r.id for r in g.requires_of(node)], ["2", "3"])

    # --- predicates ------------------------------------------------------- #

    def test_is_terminal(self):
        g = self.graph(self.issue(1, status="done"), self.issue(2, status="ongoing"))
        self.assertTrue(g.is_terminal(g.row("1")))
        self.assertFalse(g.is_terminal(g.row("2")))

    def test_is_leaf(self):
        parent = self.issue(1)
        child = self.issue(2, parent=1)
        g = self.graph(parent, child)
        self.assertFalse(g.is_leaf(parent))
        self.assertTrue(g.is_leaf(child))

    def test_is_blocked_true_for_open_dependency(self):
        g = self.graph(self.issue(1, status="ongoing"), self.issue(2, depends=[1]))
        self.assertTrue(g.is_blocked(g.row("2")))

    def test_is_blocked_false_when_dependency_terminal(self):
        g = self.graph(self.issue(1, status="done"), self.issue(2, depends=[1]))
        self.assertFalse(g.is_blocked(g.row("2")))

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
        self.assertFalse(g.is_ready(g.row("2")))

    def test_is_ready_true_once_blocker_terminal(self):
        g = self.graph(self.issue(1, status="done"), self.issue(2, depends=[1]))
        self.assertTrue(g.is_ready(g.row("2")))

    # --- dependency cycles ------------------------------------------------ #

    def test_cycles_detects_two_node_cycle(self):
        g = self.graph(self.issue(1, depends=[2]), self.issue(2, depends=[1]))
        cycles = g.cycles()
        self.assertEqual(len(cycles), 1)
        self.assertEqual(set(cycles[0]), {"1", "2"})

    def test_cycles_detects_self_loop(self):
        g = self.graph(self.issue(1, depends=[1]))
        self.assertEqual(g.cycles(), [["1"]])

    def test_cycles_empty_when_acyclic(self):
        g = self.graph(self.issue(1), self.issue(2, depends=[1]))
        self.assertEqual(g.cycles(), [])

    def test_would_cycle_true_for_self_edge(self):
        self.assertTrue(self.graph(self.issue(1)).would_cycle("1", "1"))

    def test_would_cycle_true_when_it_closes_a_loop(self):
        # 2 already depends on 1; adding 1 -> 2 would close the loop
        g = self.graph(self.issue(1), self.issue(2, depends=[1]))
        self.assertTrue(g.would_cycle("1", "2"))

    def test_would_cycle_false_for_safe_edge(self):
        g = self.graph(self.issue(1), self.issue(2), self.issue(3, depends=[1]))
        self.assertFalse(g.would_cycle("2", "1"))     # 2 -> 1 introduces no cycle

    # --- ancestor spine, match closure, sibling sort (issue #037) --------- #

    def test_ancestors_of_returns_spine_nearest_first(self):
        g = self.graph(self.issue(1), self.issue(2, parent=1),
                       self.issue(3, parent=2), self.issue(4, parent=3))
        self.assertEqual([a.id for a in g.ancestors_of(g.row("4"))], ["3", "2", "1"])

    def test_ancestors_of_empty_for_root(self):
        g = self.graph(self.issue(1))
        self.assertEqual(g.ancestors_of(g.row("1")), [])

    def test_ancestors_of_stops_at_dangling_parent(self):
        g = self.graph(self.issue(2, parent=99))   # 99 does not exist
        self.assertEqual(g.ancestors_of(g.row("2")), [])

    def test_ancestors_of_breaks_on_cycle(self):
        g = self.graph(self.issue(1, parent=2), self.issue(2, parent=1))
        # must not loop forever: returns the reachable spine, then stops
        self.assertEqual([a.id for a in g.ancestors_of(g.row("1"))], ["2"])

    def test_parent_cycles_empty_for_clean_spine(self):
        g = self.graph(self.issue(1), self.issue(2, parent=1),
                       self.issue(3, parent=2))
        self.assertEqual(g.parent_cycles(), [])

    def test_parent_cycles_empty_for_dangling_parent(self):
        g = self.graph(self.issue(2, parent=99))  # missing parent != cycle
        self.assertEqual(g.parent_cycles(), [])

    def test_parent_cycles_detects_self_parent(self):
        g = self.graph(self.issue(1, parent=1))
        self.assertEqual(g.parent_cycles(), [["1"]])

    def test_parent_cycles_detects_two_node_cycle_once(self):
        g = self.graph(self.issue(1, parent=2), self.issue(2, parent=1))
        cycles = g.parent_cycles()
        self.assertEqual(len(cycles), 1)          # one error, not one per node
        self.assertEqual(set(cycles[0]), {"1", "2"})

    def test_parent_cycles_excludes_nodes_pointing_into_a_cycle(self):
        # 4 points into the 1->2->3->1 cycle but is not itself part of it
        g = self.graph(self.issue(1, parent=2), self.issue(2, parent=3),
                       self.issue(3, parent=1), self.issue(4, parent=1))
        cycles = g.parent_cycles()
        self.assertEqual(len(cycles), 1)
        self.assertEqual(set(cycles[0]), {"1", "2", "3"})  # 4 not included

    def test_parent_cycles_reports_disjoint_cycles_separately(self):
        g = self.graph(self.issue(1, parent=2), self.issue(2, parent=1),
                       self.issue(3, parent=4), self.issue(4, parent=3))
        cycles = g.parent_cycles()
        self.assertEqual(len(cycles), 2)
        self.assertEqual(sorted(sorted(c) for c in cycles), [["1", "2"], ["3", "4"]])

    def test_match_closure_keeps_ancestor_spine_of_deep_match(self):
        g = self.graph(self.issue(1), self.issue(2, parent=1), self.issue(3, parent=2))
        shown, dim = g.match_closure(lambda r: r.id == "3")
        self.assertEqual(shown, {"1", "2", "3"})
        self.assertEqual(dim, {"1", "2"})              # ancestors shown only as context

    def test_match_closure_excludes_unrelated_nodes(self):
        g = self.graph(self.issue(1), self.issue(2, parent=1), self.issue(5))
        shown, dim = g.match_closure(lambda r: r.id == "2")
        self.assertEqual(shown, {"1", "2"})            # match + its spine
        self.assertNotIn("5", shown)                 # unrelated, no matching descendant
        self.assertEqual(dim, {"1"})

    def test_children_of_accepts_sort_key(self):
        parent = self.issue(1)
        g = self.graph(parent, self.issue(2, parent=1), self.issue(3, parent=1))
        self.assertEqual([c.id for c in g.children_of(parent)], ["2", "3"])          # default: id
        self.assertEqual([c.id for c in g.children_of(parent, key=lambda r: -int(r.id))],
                         ["3", "2"])                                                  # custom key

    # --- effective (inherited) deps + lifted cycles (issue #pzmyzv3) ------- #

    def test_subtree_includes_self_and_descendants(self):
        g = self.graph(self.issue(1), self.issue(2, parent=1),
                       self.issue(3, parent=2), self.issue(4))
        self.assertEqual(sorted(n.id for n in g.subtree(g.row("1"))), ["1", "2", "3"])
        self.assertEqual([n.id for n in g.subtree(g.row("4"))], ["4"])

    def test_subtree_safe_on_parent_cycle(self):
        g = self.graph(self.issue(1, parent=2), self.issue(2, parent=1))
        # must terminate despite the 1<->2 parent cycle
        self.assertEqual(sorted(n.id for n in g.subtree(g.row("1"))), ["1", "2"])

    def test_child_blocked_when_ancestor_dep_nonterminal(self):
        # P2 depends on P1; C2 (child of P2) inherits the block while P1 is open.
        g = self.graph(
            self.issue(1, status="ongoing"),      # P1 (non-terminal)
            self.issue(2, depends=[1]),           # P2 -> P1
            self.issue(3, parent=2),              # C2 under P2
        )
        self.assertTrue(g.is_blocked(g.row("3")))
        self.assertFalse(g.is_ready(g.row("3")))

    def test_child_ready_once_ancestor_dep_terminal(self):
        # A terminal P1 means its whole subtree is terminal (rollup) -> C2 unblocked.
        g = self.graph(
            self.issue(1, status="done"),         # P1 terminal
            self.issue(2, depends=[1]),           # P2 -> P1
            self.issue(3, parent=2),              # C2 under P2
        )
        self.assertFalse(g.is_blocked(g.row("3")))
        self.assertTrue(g.is_ready(g.row("3")))

    def test_blocking_is_one_sided(self):
        # A -> B: A is blocked by non-terminal B, but B is not blocked by A.
        g = self.graph(self.issue(1), self.issue(2, depends=[1]))
        self.assertTrue(g.is_blocked(g.row("2")))
        self.assertFalse(g.is_blocked(g.row("1")))

    def test_deep_ancestor_dep_still_blocks(self):
        # grandparent depends on X; a grandchild inherits the block.
        g = self.graph(
            self.issue(1),                        # X (non-terminal)
            self.issue(2, depends=[1]),           # GP -> X
            self.issue(3, parent=2),              # P under GP
            self.issue(4, parent=3),              # C under P
        )
        self.assertTrue(g.is_blocked(g.row("4")))

    def test_containment_detects_spine_relationship(self):
        # chain 1 <- 2 <- 3, plus a disjoint node 4.
        g = self.graph(self.issue(1), self.issue(2, parent=1),
                       self.issue(3, parent=2), self.issue(4))
        self.assertEqual(g.containment("1", "1"), "same")
        self.assertEqual(g.containment("3", "1"), "descendant")  # 3 descends 1
        self.assertEqual(g.containment("1", "3"), "ancestor")    # 1 is ancestor of 3
        self.assertEqual(g.containment("2", "3"), "ancestor")    # 2 is 3's parent
        self.assertIsNone(g.containment("2", "4"))               # disjoint subtrees

    def test_siblings_may_depend_no_false_cycle(self):
        g = self.graph(self.issue(1), self.issue(2, parent=1), self.issue(3, parent=1))
        self.assertIsNone(g.containment("2", "3"))
        self.assertFalse(g.would_cycle("2", "3"))

    def test_would_cycle_lifted_cousin_deadlock(self):
        # P2 -> P1 authored; C1(P1) -> C2(P2) closes a lifted loop, but the
        # with-the-grain edge C2 -> C1 does not.
        g = self.graph(
            self.issue(1),                        # P1
            self.issue(2, depends=[1]),           # P2 -> P1
            self.issue(3, parent=1),              # C1 under P1
            self.issue(4, parent=2),              # C2 under P2
        )
        self.assertTrue(g.would_cycle("3", "4"))
        self.assertFalse(g.would_cycle("4", "3"))

    def test_effective_cycles_detects_lifted_deadlock(self):
        g = self.graph(
            self.issue(1),                        # P1
            self.issue(2, depends=[1]),           # P2 -> P1
            self.issue(3, parent=1, depends=[4]), # C1 -> C2 (against the grain)
            self.issue(4, parent=2),              # C2
        )
        cycles = g.effective_cycles()
        self.assertTrue(cycles)
        involved = set().union(*[set(c) for c in cycles])
        self.assertTrue({"3", "4"} <= involved)

    def test_effective_cycles_flags_hand_edited_child_to_parent(self):
        # child depends on its own parent -> effective self-cycle.
        g = self.graph(self.issue(1), self.issue(2, parent=1, depends=[1]))
        self.assertTrue(g.effective_cycles())

    def test_effective_cycles_empty_for_independent_subtrees(self):
        g = self.graph(
            self.issue(1), self.issue(2, parent=1),
            self.issue(3, depends=[1]),           # authored, acyclic
            self.issue(4, parent=3),
        )
        self.assertEqual(g.effective_cycles(), [])

    def test_effective_cycles_still_catches_plain_authored_cycle(self):
        g = self.graph(self.issue(1, depends=[2]), self.issue(2, depends=[1]))
        self.assertEqual(len(g.effective_cycles()), 1)

    # --- loader ----------------------------------------------------------- #

    def test_load_graph_parallels_load_index(self):
        import io
        from contextlib import redirect_stdout
        from pathlib import Path
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_new(ns(dir=str(d), title="A", priority="high", kind=None,
                                  parent=None, points=None, depends=None, spec=None,
                                  slug=None))
            iid = Path(buf.getvalue().strip()).name.split("-")[0]
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            g = self.t.load_graph(ctx)
            self.assertEqual([r.id for r in g.rows], [iid])
            self.assertIn(iid, g.by_id)


if __name__ == "__main__":
    unittest.main()
