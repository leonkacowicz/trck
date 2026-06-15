import io
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestLifecycle(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def setup_dir(self, tmp, config=None):
        return make_tracker(tmp, config or {})

    def new(self, d, title="First", **over):
        args = ns(dir=str(d), title=title, priority="high", kind=None,
                  parent=None, depends=None, spec=None, slug=None)
        for k, v in over.items():
            setattr(args, k, v)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(args)
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def rows(self, d):
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return self.t.load_index(ctx)

    def stub_now(self, value="2026-06-12T10:00:00Z"):
        self.t.now_utc = lambda: value
        return value

    def test_new_lands_in_initial_status(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            ts = self.stub_now()
            r_id = self.new(d)
            r = self.rows(d)[0]
            self.assertEqual(r.status, "backlog")
            self.assertEqual(r.created, ts)
            self.assertIsNone(r.started)
            ctx2 = self.t.Ctx(d, self.t.load_config(d))
            self.assertTrue(self.t.issue_path(ctx2, r).exists())

    def test_mv_stamps_started_on_leaving_initial(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            ts = self.stub_now()
            r_id = self.new(d)
            self.t.cmd_mv(ns(dir=str(d), id=r_id, status="ongoing", resolution=None))
            r = self.rows(d)[0]
            self.assertEqual(r.status, "ongoing")
            self.assertEqual(r.started, ts)
            self.assertIsNone(r.closed)
            ctx2 = self.t.Ctx(d, self.t.load_config(d))
            self.assertTrue(self.t.issue_path(ctx2, r).exists())

    def test_mv_to_terminal_stamps_closed_and_resolution(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            ts = self.stub_now()
            r_id = self.new(d)
            self.t.cmd_mv(ns(dir=str(d), id=r_id, status="done", resolution="wontfix"))
            r = self.rows(d)[0]
            self.assertEqual(r.closed, ts)
            self.assertEqual(r.resolution, "wontfix")

    def test_reopen_clears_closed_and_resolution(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            r_id = self.new(d)
            self.t.cmd_mv(ns(dir=str(d), id=r_id, status="done", resolution="wontfix"))
            self.t.cmd_mv(ns(dir=str(d), id=r_id, status="ongoing", resolution=None))
            r = self.rows(d)[0]
            self.assertIsNone(r.closed)
            self.assertIsNone(r.resolution)

    def test_mv_unknown_status_dies(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            r_id = self.new(d)
            with self.assertRaises(SystemExit):
                self.t.cmd_mv(ns(dir=str(d), id=r_id, status="nope", resolution=None))

    def test_resolution_on_nonterminal_dies(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            r_id = self.new(d)
            with self.assertRaises(SystemExit):
                self.t.cmd_mv(ns(dir=str(d), id=r_id, status="ongoing", resolution="wontfix"))

    def test_new_issue_id_is_string(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            self.new(d)
            ctx = self.t.Ctx(d, self.t.load_config(d))
            rows = self.t.load_index(ctx)
            self.assertTrue(self.t.ID_RE.match(rows[0].id))
