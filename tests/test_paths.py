import io
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestPath(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title="Item", **over):
        a = dict(dir=str(d), title=title, priority="high", kind=None, parent=None,
                 points=None, depends=None, spec=None, slug=None)
        a.update(over)
        self.t.cmd_new(ns(**a))

    def cap(self, fn, args):
        buf = io.StringIO()
        with redirect_stdout(buf):
            fn(args)
        return buf.getvalue()

    def test_path_prints_absolute_path_for_id(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")                          # id 1
            out = self.cap(self.t.cmd_path, ns(dir=str(d), id=1)).strip()
            self.assertTrue(out.startswith("/"))
            self.assertTrue(out.endswith("001-alpha.md"))
            self.assertTrue(Path(out).is_file())

    def test_path_unknown_id_dies(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            with self.assertRaises(SystemExit):
                self.cap(self.t.cmd_path, ns(dir=str(d), id=99))
