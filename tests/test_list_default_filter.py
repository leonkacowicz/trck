import io
import unittest
from contextlib import redirect_stdout
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestListDefaultFilter(unittest.TestCase):
    """By default `list` hides settled work: a terminal issue is shown only when
    it is still open or sits under a non-terminal parent (progress context). The
    forest already pulls open ancestors back as dimmed context. `--all` shows
    everything; an explicit `--status` bypasses the default prune entirely."""

    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title, **over):
        a = ns(dir=str(d), title=title, priority="high", kind=None, parent=None,
               depends=None, spec=None, slug=None, points=None)
        for k, v in over.items():
            setattr(a, k, v)
        with redirect_stdout(io.StringIO()):
            self.t.cmd_new(a)

    def done(self, d, iid):
        with redirect_stdout(io.StringIO()):
            self.t.cmd_mv(ns(dir=str(d), id=iid, status="done", resolution=None))

    def list_out(self, d, status=None, all=False, flat=False):
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_list(ns(dir=str(d), status=status, kind=None, priority=None,
                               parent=None, flat=flat, id=None, all=all))
        return buf.getvalue()

    def shown(self, out, iid):
        return any(f"#{iid:03d}" in ln for ln in out.splitlines())

    def test_settled_standalone_done_is_hidden(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Open task")     # #001 stays open
            self.seed(d, "Old task")      # #002
            self.done(d, 2)               # settled, no parent
            out = self.list_out(d)
            self.assertTrue(self.shown(out, 1))   # open work stays
            self.assertFalse(self.shown(out, 2))  # settled subject pruned

    def test_done_child_of_open_epic_is_shown(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic")              # #001
            self.seed(d, "A", parent=1)       # #002
            self.seed(d, "B", parent=1)       # #003 keeps the epic open
            self.done(d, 2)                   # done under a still-open epic
            out = self.list_out(d)
            self.assertTrue(self.shown(out, 1))   # epic (non-terminal rollup)
            self.assertTrue(self.shown(out, 2))   # done child kept as context
            self.assertTrue(self.shown(out, 3))

    def test_done_child_of_fully_done_epic_collapses(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic")              # #001
            self.seed(d, "A", parent=1)       # #002
            self.seed(d, "Other")             # #003 standalone, open
            self.done(d, 2)                   # all children done -> epic rolls terminal
            out = self.list_out(d)
            self.assertFalse(self.shown(out, 1))  # settled epic gone
            self.assertFalse(self.shown(out, 2))  # its done child gone
            self.assertTrue(self.shown(out, 3))   # unrelated open work remains

    def test_deep_done_leaves_collapse_but_done_subepic_stays(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic")              # #001
            self.seed(d, "Mid", parent=1)     # #002
            self.seed(d, "g1", parent=2)      # #003
            self.seed(d, "g2", parent=2)      # #004
            self.seed(d, "Open", parent=1)    # #005 keeps the epic open
            self.done(d, 3)
            self.done(d, 4)                   # Mid's children all done -> Mid terminal
            out = self.list_out(d)
            self.assertTrue(self.shown(out, 1))   # open epic
            self.assertTrue(self.shown(out, 2))   # done sub-epic: parent open -> context
            self.assertFalse(self.shown(out, 3))  # done leaf under done parent collapses
            self.assertFalse(self.shown(out, 4))
            self.assertTrue(self.shown(out, 5))

    def test_all_flag_shows_settled_work(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Old task")          # #001
            self.done(d, 1)
            self.assertFalse(self.shown(self.list_out(d), 1))
            self.assertTrue(self.shown(self.list_out(d, all=True), 1))

    def test_explicit_status_bypasses_default_prune(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Old task")          # #001 settled standalone
            self.done(d, 1)
            out = self.list_out(d, status="done")
            self.assertTrue(self.shown(out, 1))   # explicit --status lists it

    def test_prune_applies_in_flat_view_too(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Open")              # #001
            self.seed(d, "Settled")           # #002
            self.done(d, 2)
            out = self.list_out(d, flat=True)
            self.assertTrue(self.shown(out, 1))
            self.assertFalse(self.shown(out, 2))


if __name__ == "__main__":
    unittest.main()
