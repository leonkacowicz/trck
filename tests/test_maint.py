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

    def test_normalize_strips_verbose_index(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            # write a verbose row by hand (old engine format: every key present)
            verbose = ('{"id": 1, "slug": "001-item", "title": "Item", '
                       '"kind": "task", "status": "backlog", "priority": "high", '
                       '"parent": null, "milestone": null, "depends_on": [], '
                       '"spec": null, "created": "2026-06-05", "started": null, '
                       '"closed": null, "resolution": null}\n')
            # match the actual slug the seed produced
            row = self.t.load_index(self.t.Ctx(d, self.t.load_config(d)))[0]
            verbose = verbose.replace('"001-item"', f'"{row["slug"]}"')
            (d / "index.jsonl").write_text(verbose)
            with redirect_stdout(io.StringIO()):
                self.t.cmd_normalize(ns(dir=str(d)))
            line = (d / "index.jsonl").read_text().strip()
            for stripped in ("parent", "milestone", "depends_on", "spec",
                             "started", "closed", "resolution"):
                self.assertNotIn(f'"{stripped}"', line)

    def test_normalize_is_idempotent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            with redirect_stdout(io.StringIO()):
                self.t.cmd_normalize(ns(dir=str(d)))
            first = (d / "index.jsonl").read_bytes()
            with redirect_stdout(io.StringIO()):
                self.t.cmd_normalize(ns(dir=str(d)))
            self.assertEqual((d / "index.jsonl").read_bytes(), first)

    def test_normalize_preserves_non_default_and_custom(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            ctx = self.t.Ctx(d, self.t.load_config(d))
            rows = self.t.load_index(ctx)
            rows[0]["milestone"] = "m1"
            rows[0]["custom"] = None
            self.t.save_index(ctx, rows)
            with redirect_stdout(io.StringIO()):
                self.t.cmd_normalize(ns(dir=str(d)))
            back = self.t.load_index(ctx)[0]
            self.assertEqual(back["milestone"], "m1")
            self.assertIn("custom", back)
            self.assertIsNone(back["custom"])

    def test_normalize_regenerates_summary(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            (d / "SUMMARY.md").unlink(missing_ok=True)
            with redirect_stdout(io.StringIO()):
                self.t.cmd_normalize(ns(dir=str(d)))
            self.assertTrue((d / "SUMMARY.md").exists())
