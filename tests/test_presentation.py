import io
import unittest
from contextlib import redirect_stdout
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestPresentation(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title, **over):
        a = ns(dir=str(d), title=title, priority="high", kind=None, parent=None,
               depends=None, spec=None, slug=None)
        for k, v in over.items():
            setattr(a, k, v)
        with redirect_stdout(io.StringIO()):
            self.t.cmd_new(a)

    def list_out(self, d):
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_list(ns(dir=str(d), status=None, kind=None, priority=None,
                               parent=None, flat=True, id=None))
        return buf.getvalue()

    def test_paint_emits_codes_when_enabled(self):
        self.t._use_color = lambda: True
        s = self.t.paint("hi", "red")
        self.assertTrue(s.startswith("\033[31m"))
        self.assertTrue(s.endswith("\033[0m"))
        self.assertIn("hi", s)

    def test_paint_plain_when_disabled(self):
        self.t._use_color = lambda: False
        self.assertEqual(self.t.paint("hi", "red", "bold"), "hi")

    def test_list_is_plain_when_not_a_tty(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            out = self.list_out(d)  # redirect_stdout -> StringIO is not a tty
            self.assertNotIn("\033[", out)  # no ANSI escapes
            self.assertIn("#1", out)
            self.assertIn("Alpha", out)

    def row(self, iid=1, title="Alpha", **over):
        base = dict(id=iid, slug=f"i{iid}", title=title, kind="task",
                    status="backlog", priority="high")
        base.update(over)
        return self.t.Issue(**base)

    def render(self, d, rows, **kw):
        ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.print_rows(ctx, rows, **kw)
        return buf.getvalue()

    def test_print_rows_prefix_sits_before_title(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            out = self.render(d, [self.row(title="Alpha")], prefix=lambda r: "|- ")
            self.assertIn("|- Alpha", out)

    def test_print_rows_default_has_no_prefix(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            out = self.render(d, [self.row(title="Alpha")])
            self.assertIn(" Alpha", out)
            self.assertNotIn("|-", out)

    def test_list_columns_align_with_custom_statuses(self):
        cfg = {"statuses": [{"name": "todo", "role": "initial"},
                            {"name": "in-progress"},
                            {"name": "shipped", "role": "terminal"}]}
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, cfg)
            self.seed(d, "A")
            self.seed(d, "B")
            self.t.cmd_mv(ns(dir=str(d), id=2, status="in-progress", resolution=None))
            lines = [ln for ln in self.list_out(d).splitlines() if ln.strip()]
            # priority column starts at the same offset on every row (status padded to max width)
            offsets = [ln.index("high") for ln in lines]
            self.assertEqual(len(set(offsets)), 1)

    def test_row_renders_id_without_zero_padding(self):
        t = self.t
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            ctx = t.build_ctx_or_die(ns(dir=str(d)))
            row = t.Issue(id="7", slug="s", title="T", kind="task",
                          status="backlog", priority="high")
            line = t.node_label(ctx, row, focal=False)
            self.assertIn("#7", line)
            self.assertNotIn("#007", line)
