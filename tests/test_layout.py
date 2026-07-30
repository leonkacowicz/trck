"""The flat items/ layout: status lives only in index.jsonl, never in the path."""
import io
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestFlatLayout(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title="Item", **over):
        a = dict(dir=str(d), title=title, priority="high", kind=None, parent=None,
                 points=None, depends=None, spec=None, slug=None, pr=None)
        a.update(over)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(ns(**a))
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def test_new_writes_into_items_dir(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            files = sorted(p.name for p in (d / "items").glob("*.md"))
            self.assertEqual(len(files), 1)
            self.assertTrue(files[0].endswith("-alpha.md"))
            self.assertFalse((d / "backlog").exists())

    def test_status_change_does_not_move_the_file(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            before = sorted(p.name for p in (d / "items").glob("*.md"))
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_mv(ns(dir=str(d), id=iid, status="done",
                                 resolution=None, pr=None))
            after = sorted(p.name for p in (d / "items").glob("*.md"))
            self.assertEqual(before, after)
            self.assertFalse((d / "done").exists())
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            row = self.t.get_row(self.t.load_index(ctx), iid)
            self.assertEqual(row.status, "done")

    def test_rel_link_points_into_items_dir(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            row = self.t.get_row(self.t.load_index(ctx), iid)
            self.assertEqual(self.t.rel_link(row), f"items/{self.t.filename(row)}")
            self.assertIn(f"(items/{self.t.filename(row)})",
                          (d / "SUMMARY.md").read_text())

    def test_slug_change_still_renames_within_items(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_set(ns(dir=str(d), id=iid, slug="renamed", title=None,
                                  priority=None, points=None, parent=None, spec=None,
                                  pr=None, kind=None, field=None, unset=None, auto=False))
            files = sorted(p.name for p in (d / "items").glob("*.md"))
            self.assertEqual(files, [f"{iid}-renamed.md"])

    def test_scan_files_maps_id_to_slug_and_filename(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            found = self.t.scan_files(ctx)
            self.assertEqual(found[iid], ("alpha", f"{iid}-alpha.md"))
