"""Status rollup (#67): reconcile rule, normalize_statuses cascade, manual_status."""
import io
import unittest
from contextlib import redirect_stdout
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestReconcileRule(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.cfg = self.t.DEFAULT_CONFIG  # backlog(initial) / ongoing(active) / done(terminal)

    def r(self, *child_statuses):
        return self.t.reconcile(self.cfg, list(child_statuses))

    def test_all_initial_rolls_up_to_initial(self):
        self.assertEqual(self.r("backlog", "backlog"), "backlog")

    def test_all_terminal_rolls_up_to_terminal(self):
        self.assertEqual(self.r("done", "done", "done"), "done")

    def test_any_active_child_rolls_up_to_active(self):
        self.assertEqual(self.r("backlog", "ongoing"), "ongoing")

    def test_partial_completion_rolls_up_to_active(self):
        # a mix of initial + terminal, with no active child, is still "in progress"
        self.assertEqual(self.r("backlog", "done"), "ongoing")

    def test_single_child_mirrors_its_lifecycle_position(self):
        self.assertEqual(self.r("backlog"), "backlog")   # all initial
        self.assertEqual(self.r("ongoing"), "ongoing")   # active
        self.assertEqual(self.r("done"), "done")         # all terminal


class TestStatusCascade(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.t.now_utc = lambda: "2026-06-12T10:00:00Z"

    def new(self, d, title="I", parent=None):
        self.t.cmd_new(ns(dir=str(d), title=title, priority="high", kind=None,
                          parent=parent, depends=None, spec=None, slug=None))

    def mv(self, d, iid, status, resolution=None):
        self.t.cmd_mv(ns(dir=str(d), id=iid, status=status, resolution=resolution))

    def status(self, d, iid):
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return self.t.get_row(self.t.load_index(ctx), iid).status

    def row(self, d, iid):
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return self.t.get_row(self.t.load_index(ctx), iid)

    def test_activation_cascades_to_parent_and_grandparent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "gp")                 # 1
            self.new(d, "p", parent=1)        # 2
            self.new(d, "leaf", parent=2)     # 3
            self.mv(d, 3, "ongoing")
            self.assertEqual(self.status(d, 3), "ongoing")
            self.assertEqual(self.status(d, 2), "ongoing")
            self.assertEqual(self.status(d, 1), "ongoing")
            self.assertTrue((d / "ongoing" / "001-gp.md").exists())

    def test_completion_cascades_to_terminal(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "gp"); self.new(d, "p", parent=1); self.new(d, "leaf", parent=2)
            self.mv(d, 3, "done")
            self.assertEqual(self.status(d, 2), "done")
            self.assertEqual(self.status(d, 1), "done")
            self.assertEqual(self.row(d, 1).closed, "2026-06-12T10:00:00Z")
            self.assertIsNone(self.row(d, 1).resolution)  # genuine rollup completion

    def test_partial_completion_keeps_parent_active(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "p"); self.new(d, "a", parent=1); self.new(d, "b", parent=1)
            self.mv(d, 2, "done")
            self.assertEqual(self.status(d, 1), "ongoing")  # b still open

    def test_reopening_a_child_reopens_a_completed_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "p"); self.new(d, "leaf", parent=1)
            self.mv(d, 2, "done")
            self.assertEqual(self.status(d, 1), "done")
            self.mv(d, 2, "ongoing")            # reopen the only child
            self.assertEqual(self.status(d, 1), "ongoing")
            self.assertIsNone(self.row(d, 1).closed)
            self.assertIsNone(self.row(d, 1).resolution)

    def test_all_children_back_to_initial_returns_parent_to_initial(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "p"); self.new(d, "leaf", parent=1)
            self.mv(d, 2, "ongoing")
            self.assertEqual(self.status(d, 1), "ongoing")
            self.mv(d, 2, "backlog")
            self.assertEqual(self.status(d, 1), "backlog")

    def test_adding_a_child_under_a_done_parent_reopens_it(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "p"); self.new(d, "leaf", parent=1)
            self.mv(d, 2, "done")
            self.assertEqual(self.status(d, 1), "done")
            self.new(d, "fresh", parent=1)        # new backlog child
            self.assertEqual(self.status(d, 1), "ongoing")

    def test_reparenting_reconciles_both_old_and_new_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "p1")                     # 1
            self.new(d, "a", parent=1)            # 2
            self.new(d, "b", parent=1)            # 3
            self.new(d, "p2")                     # 4
            self.new(d, "c", parent=4)            # 5
            self.mv(d, 2, "done")                 # p1: a done, b backlog -> ongoing
            self.mv(d, 5, "done")                 # p2: c done -> done
            self.assertEqual(self.status(d, 1), "ongoing")
            self.assertEqual(self.status(d, 4), "done")
            # move b from p1 to p2
            self.t.cmd_set(ns(dir=str(d), id=3, parent="4", priority=None, points=None,
                              spec=None, kind=None, field=None, unset=None, slug=None,
                              title=None, auto=False))
            self.assertEqual(self.status(d, 1), "done")     # p1 now has only the done child
            self.assertEqual(self.status(d, 4), "ongoing")  # p2 now has a backlog child


class TestManualOverride(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.t.now_utc = lambda: "2026-06-12T10:00:00Z"

    def new(self, d, title="I", parent=None):
        self.t.cmd_new(ns(dir=str(d), title=title, priority="high", kind=None,
                          parent=parent, depends=None, spec=None, slug=None))

    def mv(self, d, iid, status, resolution=None):
        self.t.cmd_mv(ns(dir=str(d), id=iid, status=status, resolution=resolution))

    def row(self, d, iid):
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return self.t.get_row(self.t.load_index(ctx), iid)

    def test_moving_a_parent_against_its_children_pins_it(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "p"); self.new(d, "leaf", parent=1)   # leaf backlog
            self.mv(d, 1, "done")                             # diverges from reconcile=backlog
            self.assertTrue(self.row(d, 1).manual_status)
            self.assertEqual(self.row(d, 1).status, "done")   # override sticks
            self.assertEqual(self.row(d, 2).status, "backlog")

    def test_moving_a_parent_in_agreement_does_not_pin(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "p"); self.new(d, "leaf", parent=1)
            self.mv(d, 2, "ongoing")                          # p derives ongoing
            self.mv(d, 1, "ongoing")                          # agrees with reconcile
            self.assertFalse(self.row(d, 1).manual_status)

    def test_pinned_parent_is_not_re_derived_by_later_child_moves(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "p"); self.new(d, "leaf", parent=1)
            self.mv(d, 1, "done")                             # pin p done
            self.mv(d, 2, "ongoing")                          # would normally reopen p
            self.assertTrue(self.row(d, 1).manual_status)
            self.assertEqual(self.row(d, 1).status, "done")   # stays pinned

    def test_leaf_move_never_sets_manual_status(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "solo")
            self.mv(d, 1, "ongoing")
            self.assertFalse(self.row(d, 1).manual_status)

    def test_pinned_parent_status_still_feeds_its_own_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "gp"); self.new(d, "p", parent=1); self.new(d, "leaf", parent=2)
            self.mv(d, 2, "done")                             # pin p done (leaf still backlog)
            self.assertTrue(self.row(d, 2).manual_status)
            self.assertEqual(self.row(d, 1).status, "done")  # gp derives from p's pinned done

    def set_(self, d, iid, **over):
        args = dict(dir=str(d), id=iid, priority=None, points=None, parent=None,
                    spec=None, kind=None, field=None, unset=None, slug=None,
                    title=None, auto=False)
        args.update(over)
        self.t.cmd_set(ns(**args))

    def test_set_auto_clears_pin_and_re_derives(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "p"); self.new(d, "leaf", parent=1)
            self.mv(d, 1, "done")                             # pin p done; leaf backlog
            self.assertTrue(self.row(d, 1).manual_status)
            self.set_(d, 1, auto=True)                        # return p to derivation
            self.assertFalse(self.row(d, 1).manual_status)
            self.assertEqual(self.row(d, 1).status, "backlog")  # re-derives from leaf


class TestPresentation(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.t.now_utc = lambda: "2026-06-12T10:00:00Z"

    def new(self, d, title="I", parent=None):
        self.t.cmd_new(ns(dir=str(d), title=title, priority="high", kind=None,
                          parent=parent, depends=None, spec=None, slug=None))

    def mv(self, d, iid, status, resolution=None):
        self.t.cmd_mv(ns(dir=str(d), id=iid, status=status, resolution=resolution))

    def show(self, d, iid):
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_show(ns(dir=str(d), id=iid, json=False))
        return buf.getvalue()

    def test_active_status_keeps_in_progress_icon(self):
        cfg = self.t.DEFAULT_CONFIG
        ctx = self.t.Ctx("/tmp/x", cfg)
        self.assertEqual(self.t.status_icon(ctx, "ongoing"), "◐")

    def test_show_marks_a_pinned_issue(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "p"); self.new(d, "leaf", parent=1)
            self.mv(d, 1, "done")  # pin p
            self.assertIn("manual_status", self.show(d, 1))

    def test_show_omits_manual_status_when_unset(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "solo")
            self.assertNotIn("manual_status", self.show(d, 1))
