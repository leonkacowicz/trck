import io
import unittest
from contextlib import redirect_stdout
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestListProgress(unittest.TestCase):
    """`list` surfaces the points-weighted completion rollup (#019) on parent rows."""

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

    def list_out(self, d, flat=True):
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_list(ns(dir=str(d), status=None, kind=None, priority=None,
                               parent=None, flat=flat, id=None))
        return buf.getvalue()

    def line(self, out, iid):
        for ln in out.splitlines():
            if f"#{iid}" in ln:
                return ln
        return ""

    def test_parent_shows_pct_leaf_shows_none(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic")                 # #001 parent
            self.seed(d, "A", parent=1)          # #002 leaf
            self.seed(d, "B", parent=1)          # #003 leaf
            self.done(d, 2)                      # 1 of 2 default-weight leaves done
            out = self.list_out(d)
            self.assertIn("50%", self.line(out, 1))   # parent rolls up
            self.assertNotIn("%", self.line(out, 2))  # leaves carry no rollup
            self.assertNotIn("%", self.line(out, 3))

    def test_pct_is_weighted_by_points_not_a_count(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic")                         # #001 parent
            self.seed(d, "light", parent=1, points=1)    # #002
            self.seed(d, "heavy", parent=1, points=3)    # #003
            self.done(d, 3)                              # 3 of 4 pts done
            out = self.list_out(d)
            self.assertIn("75%", self.line(out, 1))      # weighted, not the 50% a count gives

    def test_pct_sums_deep_leaf_descendants(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic")                 # #001
            self.seed(d, "Mid", parent=1)        # #002 becomes a parent
            self.seed(d, "g1", parent=2)         # #003 leaf
            self.seed(d, "g2", parent=2)         # #004 leaf
            self.done(d, 3)                      # 1 of 2 grandchildren done
            out = self.list_out(d)
            self.assertIn("50%", self.line(out, 1))   # top epic sums grandchildren

    def test_leaf_returns_empty_parent_returns_pct(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic")
            self.seed(d, "A", parent=1)
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            g = self.t.load_graph(ctx)
            self.assertEqual(self.t.progress_pct(g, g.row("2")), "")     # leaf
            self.assertEqual(self.t.progress_pct(g, g.row("1")), " 0%")  # parent, nothing done

    def test_pct_zero_when_no_leaf_points(self):
        # A degenerate self-parent yields a cycle-guarded empty rollup (ptotal == 0);
        # the display must fall back to 0%, never divide by zero.
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            row = self.t.Issue(id=1, slug="x", title="X", kind="task",
                               status="backlog", priority="high", parent=1)
            g = self.t.Graph(ctx.cfg, [row])
            self.assertEqual(self.t.progress_pct(g, row), " 0%")


if __name__ == "__main__":
    unittest.main()
