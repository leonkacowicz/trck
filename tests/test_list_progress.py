import io
import unittest
from contextlib import redirect_stdout
from pathlib import Path
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
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(a)
        return Path(buf.getvalue().strip()).name.split("-")[0]

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
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "A", parent=id1)
            id3 = self.seed(d, "B", parent=id1)
            self.done(d, id2)                        # 1 of 2 default-weight leaves done
            out = self.list_out(d)
            self.assertIn("50%", self.line(out, id1))   # parent rolls up
            self.assertNotIn("%", self.line(out, id2))  # leaves carry no rollup
            self.assertNotIn("%", self.line(out, id3))

    def test_pct_is_weighted_by_points_not_a_count(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "light", parent=id1, points=1)
            id3 = self.seed(d, "heavy", parent=id1, points=3)
            self.done(d, id3)                            # 3 of 4 pts done
            out = self.list_out(d)
            self.assertIn("75%", self.line(out, id1))    # weighted, not the 50% a count gives

    def test_pct_sums_deep_leaf_descendants(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Mid", parent=id1)
            id3 = self.seed(d, "g1", parent=id2)
            id4 = self.seed(d, "g2", parent=id2)
            self.done(d, id3)                        # 1 of 2 grandchildren done
            out = self.list_out(d)
            self.assertIn("50%", self.line(out, id1))   # top epic sums grandchildren

    def test_leaf_returns_empty_parent_returns_pct(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "A", parent=id1)
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            g = self.t.load_graph(ctx)
            self.assertEqual(self.t.progress_pct(g, g.row(id2)), "")      # leaf
            self.assertEqual(self.t.progress_pct(g, g.row(id1)), " 0%")   # parent, nothing done

    def test_pct_zero_when_no_leaf_points(self):
        # A degenerate self-parent yields a cycle-guarded empty rollup (ptotal == 0);
        # the display must fall back to 0%, never divide by zero.
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            row = self.t.Issue(id="x", slug="x", title="X", kind="task",
                               status="backlog", priority="high", parent="x")
            g = self.t.Graph(ctx.cfg, [row])
            self.assertEqual(self.t.progress_pct(g, row), " 0%")


if __name__ == "__main__":
    unittest.main()
