import unittest
from datetime import date
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
        self.t.cmd_new(args)

    def rows(self, d):
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return self.t.load_index(ctx)

    def test_new_lands_in_initial_status(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            self.new(d)
            r = self.rows(d)[0]
            self.assertEqual(r.status, "backlog")
            self.assertEqual(r.created, date.today().isoformat())
            self.assertIsNone(r.started)
            self.assertTrue((d / "backlog" / "001-first.md").exists())

    def test_mv_stamps_started_on_leaving_initial(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            self.new(d)
            self.t.cmd_mv(ns(dir=str(d), id=1, status="ongoing", resolution=None))
            r = self.rows(d)[0]
            self.assertEqual(r.status, "ongoing")
            self.assertEqual(r.started, date.today().isoformat())
            self.assertIsNone(r.closed)
            self.assertTrue((d / "ongoing" / "001-first.md").exists())

    def test_mv_to_terminal_stamps_closed_and_resolution(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            self.new(d)
            self.t.cmd_mv(ns(dir=str(d), id=1, status="done", resolution="wontfix"))
            r = self.rows(d)[0]
            self.assertEqual(r.closed, date.today().isoformat())
            self.assertEqual(r.resolution, "wontfix")

    def test_reopen_clears_closed_and_resolution(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            self.new(d)
            self.t.cmd_mv(ns(dir=str(d), id=1, status="done", resolution="wontfix"))
            self.t.cmd_mv(ns(dir=str(d), id=1, status="ongoing", resolution=None))
            r = self.rows(d)[0]
            self.assertIsNone(r.closed)
            self.assertIsNone(r.resolution)

    def test_mv_unknown_status_dies(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            self.new(d)
            with self.assertRaises(SystemExit):
                self.t.cmd_mv(ns(dir=str(d), id=1, status="nope", resolution=None))

    def test_resolution_on_nonterminal_dies(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            self.new(d)
            with self.assertRaises(SystemExit):
                self.t.cmd_mv(ns(dir=str(d), id=1, status="ongoing", resolution="wontfix"))
