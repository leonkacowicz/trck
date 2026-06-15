"""Status rollup (#67): reconcile rule, normalize_statuses cascade, manual_status."""
import io
import unittest
from contextlib import redirect_stdout
from pathlib import Path
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
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(ns(dir=str(d), title=title, priority="high", kind=None,
                              parent=parent, depends=None, spec=None, slug=None))
        return Path(buf.getvalue().strip()).name.split("-")[0]

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
            id_gp = self.new(d, "gp")
            id_p = self.new(d, "p", parent=id_gp)
            id_leaf = self.new(d, "leaf", parent=id_p)
            self.mv(d, id_leaf, "ongoing")
            self.assertEqual(self.status(d, id_leaf), "ongoing")
            self.assertEqual(self.status(d, id_p), "ongoing")
            self.assertEqual(self.status(d, id_gp), "ongoing")

    def test_completion_cascades_to_terminal(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id_gp = self.new(d, "gp")
            id_p = self.new(d, "p", parent=id_gp)
            id_leaf = self.new(d, "leaf", parent=id_p)
            self.mv(d, id_leaf, "done")
            self.assertEqual(self.status(d, id_p), "done")
            self.assertEqual(self.status(d, id_gp), "done")
            self.assertEqual(self.row(d, id_gp).closed, "2026-06-12T10:00:00Z")
            self.assertIsNone(self.row(d, id_gp).resolution)  # genuine rollup completion

    def test_partial_completion_keeps_parent_active(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id_p = self.new(d, "p")
            id_a = self.new(d, "a", parent=id_p)
            id_b = self.new(d, "b", parent=id_p)
            self.mv(d, id_a, "done")
            self.assertEqual(self.status(d, id_p), "ongoing")  # b still open

    def test_reopening_a_child_reopens_a_completed_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id_p = self.new(d, "p")
            id_leaf = self.new(d, "leaf", parent=id_p)
            self.mv(d, id_leaf, "done")
            self.assertEqual(self.status(d, id_p), "done")
            self.mv(d, id_leaf, "ongoing")            # reopen the only child
            self.assertEqual(self.status(d, id_p), "ongoing")
            self.assertIsNone(self.row(d, id_p).closed)
            self.assertIsNone(self.row(d, id_p).resolution)

    def test_all_children_back_to_initial_returns_parent_to_initial(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id_p = self.new(d, "p")
            id_leaf = self.new(d, "leaf", parent=id_p)
            self.mv(d, id_leaf, "ongoing")
            self.assertEqual(self.status(d, id_p), "ongoing")
            self.mv(d, id_leaf, "backlog")
            self.assertEqual(self.status(d, id_p), "backlog")

    def test_adding_a_child_under_a_done_parent_reopens_it(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id_p = self.new(d, "p")
            id_leaf = self.new(d, "leaf", parent=id_p)
            self.mv(d, id_leaf, "done")
            self.assertEqual(self.status(d, id_p), "done")
            self.new(d, "fresh", parent=id_p)        # new backlog child
            self.assertEqual(self.status(d, id_p), "ongoing")

    def test_reparenting_reconciles_both_old_and_new_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id_p1 = self.new(d, "p1")
            id_a = self.new(d, "a", parent=id_p1)
            id_b = self.new(d, "b", parent=id_p1)
            id_p2 = self.new(d, "p2")
            id_c = self.new(d, "c", parent=id_p2)
            self.mv(d, id_a, "done")                 # p1: a done, b backlog -> ongoing
            self.mv(d, id_c, "done")                 # p2: c done -> done
            self.assertEqual(self.status(d, id_p1), "ongoing")
            self.assertEqual(self.status(d, id_p2), "done")
            # move b from p1 to p2
            self.t.cmd_set(ns(dir=str(d), id=id_b, parent=id_p2, priority=None, points=None,
                              spec=None, kind=None, field=None, unset=None, slug=None,
                              title=None, auto=False))
            self.assertEqual(self.status(d, id_p1), "done")     # p1 now has only the done child
            self.assertEqual(self.status(d, id_p2), "ongoing")  # p2 now has a backlog child


class TestManualOverride(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.t.now_utc = lambda: "2026-06-12T10:00:00Z"

    def new(self, d, title="I", parent=None):
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(ns(dir=str(d), title=title, priority="high", kind=None,
                              parent=parent, depends=None, spec=None, slug=None))
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def mv(self, d, iid, status, resolution=None):
        self.t.cmd_mv(ns(dir=str(d), id=iid, status=status, resolution=resolution))

    def row(self, d, iid):
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return self.t.get_row(self.t.load_index(ctx), iid)

    def test_moving_a_parent_against_its_children_pins_it(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id_p = self.new(d, "p")
            id_leaf = self.new(d, "leaf", parent=id_p)   # leaf backlog
            self.mv(d, id_p, "done")                      # diverges from reconcile=backlog
            self.assertTrue(self.row(d, id_p).manual_status)
            self.assertEqual(self.row(d, id_p).status, "done")   # override sticks
            self.assertEqual(self.row(d, id_leaf).status, "backlog")

    def test_moving_a_parent_in_agreement_does_not_pin(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id_p = self.new(d, "p")
            id_leaf = self.new(d, "leaf", parent=id_p)
            self.mv(d, id_leaf, "ongoing")                # p derives ongoing
            self.mv(d, id_p, "ongoing")                   # agrees with reconcile
            self.assertFalse(self.row(d, id_p).manual_status)

    def test_pinned_parent_is_not_re_derived_by_later_child_moves(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id_p = self.new(d, "p")
            id_leaf = self.new(d, "leaf", parent=id_p)
            self.mv(d, id_p, "done")                      # pin p done
            self.mv(d, id_leaf, "ongoing")                # would normally reopen p
            self.assertTrue(self.row(d, id_p).manual_status)
            self.assertEqual(self.row(d, id_p).status, "done")   # stays pinned

    def test_leaf_move_never_sets_manual_status(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id_solo = self.new(d, "solo")
            self.mv(d, id_solo, "ongoing")
            self.assertFalse(self.row(d, id_solo).manual_status)

    def test_pinned_parent_status_still_feeds_its_own_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id_gp = self.new(d, "gp")
            id_p = self.new(d, "p", parent=id_gp)
            id_leaf = self.new(d, "leaf", parent=id_p)
            self.mv(d, id_p, "done")                      # pin p done (leaf still backlog)
            self.assertTrue(self.row(d, id_p).manual_status)
            self.assertEqual(self.row(d, id_gp).status, "done")  # gp derives from p's pinned done

    def set_(self, d, iid, **over):
        args = dict(dir=str(d), id=iid, priority=None, points=None, parent=None,
                    spec=None, kind=None, field=None, unset=None, slug=None,
                    title=None, auto=False)
        args.update(over)
        self.t.cmd_set(ns(**args))

    def test_set_auto_clears_pin_and_re_derives(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id_p = self.new(d, "p")
            id_leaf = self.new(d, "leaf", parent=id_p)
            self.mv(d, id_p, "done")                      # pin p done; leaf backlog
            self.assertTrue(self.row(d, id_p).manual_status)
            self.set_(d, id_p, auto=True)                 # return p to derivation
            self.assertFalse(self.row(d, id_p).manual_status)
            self.assertEqual(self.row(d, id_p).status, "backlog")  # re-derives from leaf


class TestPresentation(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.t.now_utc = lambda: "2026-06-12T10:00:00Z"

    def new(self, d, title="I", parent=None):
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(ns(dir=str(d), title=title, priority="high", kind=None,
                              parent=parent, depends=None, spec=None, slug=None))
        return Path(buf.getvalue().strip()).name.split("-")[0]

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
            id_p = self.new(d, "p")
            id_leaf = self.new(d, "leaf", parent=id_p)
            self.mv(d, id_p, "done")  # pin p
            self.assertIn("manual_status", self.show(d, id_p))

    def test_show_omits_manual_status_when_unset(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id_solo = self.new(d, "solo")
            self.assertNotIn("manual_status", self.show(d, id_solo))
