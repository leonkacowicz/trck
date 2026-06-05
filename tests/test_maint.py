import io
import unittest
from contextlib import redirect_stdout
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestMaint(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title="Item"):
        self.t.cmd_new(ns(dir=str(d), title=title, priority="high", epic=False,
                          parent=None, milestone=None, depends=None, spec=None, slug=None))

    def test_check_passes_clean_tracker(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_check(ns(dir=str(d)))   # should not raise
            self.assertIn("OK", buf.getvalue())

    def test_check_exits_nonzero_on_error(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            # corrupt: index row whose file we delete
            (d / "backlog" / "001-item.md").unlink()
            with self.assertRaises(SystemExit) as cm:
                with redirect_stdout(io.StringIO()):
                    self.t.cmd_check(ns(dir=str(d)))
            self.assertNotEqual(cm.exception.code, 0)

    def test_summary_writes_file(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            with redirect_stdout(io.StringIO()):
                self.t.cmd_summary(ns(dir=str(d)))
            self.assertTrue((d / "SUMMARY.md").exists())
