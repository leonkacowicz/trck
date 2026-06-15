import io
import unittest
from contextlib import redirect_stdout
from pathlib import Path
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
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(a)
        return Path(buf.getvalue().strip()).name.split("-")[0]

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
        return any(f"#{iid}" in ln for ln in out.splitlines())

    def test_settled_standalone_done_is_hidden(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Open task")
            id2 = self.seed(d, "Old task")
            self.done(d, id2)               # settled, no parent
            out = self.list_out(d)
            self.assertTrue(self.shown(out, id1))   # open work stays
            self.assertFalse(self.shown(out, id2))  # settled subject pruned

    def test_done_child_of_open_epic_is_shown(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "A", parent=id1)
            id3 = self.seed(d, "B", parent=id1)      # keeps the epic open
            self.done(d, id2)                         # done under a still-open epic
            out = self.list_out(d)
            self.assertTrue(self.shown(out, id1))     # epic (non-terminal rollup)
            self.assertTrue(self.shown(out, id2))     # done child kept as context
            self.assertTrue(self.shown(out, id3))

    def test_done_child_of_fully_done_epic_collapses(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "A", parent=id1)
            id3 = self.seed(d, "Other")               # standalone, open
            self.done(d, id2)                         # all children done -> epic rolls terminal
            out = self.list_out(d)
            self.assertFalse(self.shown(out, id1))    # settled epic gone
            self.assertFalse(self.shown(out, id2))    # its done child gone
            self.assertTrue(self.shown(out, id3))     # unrelated open work remains

    def test_deep_done_leaves_collapse_but_done_subepic_stays(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Mid", parent=id1)
            id3 = self.seed(d, "g1", parent=id2)
            id4 = self.seed(d, "g2", parent=id2)
            id5 = self.seed(d, "Open", parent=id1)   # keeps the epic open
            self.done(d, id3)
            self.done(d, id4)                         # Mid's children all done -> Mid terminal
            out = self.list_out(d)
            self.assertTrue(self.shown(out, id1))     # open epic
            self.assertTrue(self.shown(out, id2))     # done sub-epic: parent open -> context
            self.assertFalse(self.shown(out, id3))    # done leaf under done parent collapses
            self.assertFalse(self.shown(out, id4))
            self.assertTrue(self.shown(out, id5))

    def test_all_flag_shows_settled_work(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Old task")
            self.done(d, id1)
            self.assertFalse(self.shown(self.list_out(d), id1))
            self.assertTrue(self.shown(self.list_out(d, all=True), id1))

    def test_explicit_status_bypasses_default_prune(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Old task")            # settled standalone
            self.done(d, id1)
            out = self.list_out(d, status="done")
            self.assertTrue(self.shown(out, id1))     # explicit --status lists it

    def test_prune_applies_in_flat_view_too(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Open")
            id2 = self.seed(d, "Settled")
            self.done(d, id2)
            out = self.list_out(d, flat=True)
            self.assertTrue(self.shown(out, id1))
            self.assertFalse(self.shown(out, id2))


if __name__ == "__main__":
    unittest.main()
