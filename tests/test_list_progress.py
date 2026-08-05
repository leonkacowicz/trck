import io
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestListProgress(unittest.TestCase):
    """`progress_pct`, the points-weighted completion rollup shown on parent rows.

    What the rollup *renders* is specified by the conformance fixtures
    (`list-rollup-is-weighted-by-points`, `list-rollup-sums-deep-leaf-descendants`,
    `list-shows-a-parent-rollup-percent`). What is left here is the helper itself, at
    two points the CLI cannot reach: the leaf/parent distinction in its return value,
    and a degenerate graph whose cycle guard leaves no leaf points to divide by.
    """

    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title, **over):
        a = ns(dir=str(d), title=title, priority="high", parent=None,
               depends=None, spec=None, slug=None, points=None)
        for k, v in over.items():
            setattr(a, k, v)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(a)
        return Path(buf.getvalue().strip()).name.split("-")[0]

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
            row = self.t.Issue(id="x", slug="x", title="X",
                               status="backlog", priority="high", parent="x")
            g = self.t.Graph(ctx.cfg, [row])
            self.assertEqual(self.t.progress_pct(g, row), " 0%")


if __name__ == "__main__":
    unittest.main()
