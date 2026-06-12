import json
import unittest
from io import StringIO
from contextlib import redirect_stdout
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestTimestampDisplay(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def setup_dir(self, tmp):
        return make_tracker(tmp, {})

    def new(self, d, title="First"):
        self.t.cmd_new(ns(dir=str(d), title=title, priority="high", kind=None,
                          parent=None, depends=None, spec=None, slug=None))

    def rows(self, d):
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return self.t.load_index(ctx)

    def test_date_slice_trims_timestamp_to_date(self):
        self.assertEqual(self.t.date_slice("2026-06-12T10:00:00Z"), "2026-06-12")

    def test_date_slice_passes_through_legacy_date(self):
        self.assertEqual(self.t.date_slice("2026-06-05"), "2026-06-05")

    def test_date_slice_handles_none(self):
        self.assertEqual(self.t.date_slice(None), "")

    def test_summary_shows_bare_date_for_closed(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            self.t.now_utc = lambda: "2026-06-12T10:00:00Z"
            self.new(d)
            self.t.cmd_mv(ns(dir=str(d), id=1, status="done", resolution=None))
            text = (d / "SUMMARY.md").read_text()
            self.assertIn("(closed 2026-06-12)", text)
            self.assertNotIn("2026-06-12T10:00:00Z", text)

    def test_show_human_view_slices_dates_but_json_keeps_full(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            self.t.now_utc = lambda: "2026-06-12T10:00:00Z"
            self.new(d)
            # human view: bare date
            buf = StringIO()
            with redirect_stdout(buf):
                self.t.cmd_show(ns(dir=str(d), id=1, json=False))
            human = buf.getvalue()
            self.assertRegex(human, r"created\s+2026-06-12\b")
            self.assertNotIn("2026-06-12T10:00:00Z", human)
            # json view: full timestamp (parse only the JSON object, which
            # precedes the body section that show always appends)
            buf = StringIO()
            with redirect_stdout(buf):
                self.t.cmd_show(ns(dir=str(d), id=1, json=True))
            jtext = buf.getvalue().split("\n--- body ---", 1)[0]
            payload = json.loads(jtext)
            self.assertEqual(payload["created"], "2026-06-12T10:00:00Z")


class TestTimestampBackCompat(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def setup_dir(self, tmp):
        return make_tracker(tmp, {})

    def write_index(self, d, lines):
        (d / "index.jsonl").write_text("".join(json.dumps(x) + "\n" for x in lines))

    def ctx(self, d):
        return self.t.Ctx(d, self.t.load_config(d))

    def test_legacy_date_only_loads_check_clean_and_normalize_preserves(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            (d / "backlog").mkdir()
            # create the issue body file the index points at
            (d / "backlog" / "001-legacy.md").write_text("# Legacy\n")
            self.write_index(d, [{
                "id": 1, "slug": "legacy", "title": "Legacy", "kind": "task",
                "status": "backlog", "priority": "medium",
                "created": "2026-06-05",  # legacy day-only value
            }])
            ctx = self.ctx(d)
            rows = self.t.load_index(ctx)          # loads without error
            self.assertEqual(rows[0].created, "2026-06-05")
            errors, _ = self.t.validate(ctx, rows)  # check passes
            self.assertEqual(errors, [])
            # normalize must NOT expand the date-only value
            self.t.cmd_normalize(ns(dir=str(d)))
            reloaded = self.t.load_index(self.ctx(d))
            self.assertEqual(reloaded[0].created, "2026-06-05")

    def test_mixed_form_sort_orders_date_before_same_day_timestamp(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            (d / "backlog").mkdir()
            (d / "backlog" / "001-older.md").write_text("# Older\n")
            (d / "backlog" / "002-newer.md").write_text("# Newer\n")
            self.write_index(d, [
                {"id": 1, "slug": "older", "title": "Older", "kind": "task",
                 "status": "backlog", "priority": "medium", "created": "2026-06-05"},
                {"id": 2, "slug": "newer", "title": "Newer", "kind": "task",
                 "status": "backlog", "priority": "medium",
                 "created": "2026-06-05T09:00:00Z"},
            ])
            # Drive the real engine sort path rather than a reimplemented key.
            # "2026-06-05" < "2026-06-05T09:00:00Z" lexicographically, so the
            # legacy date-only row (#001 Older) must appear before #002 Newer.
            buf = StringIO()
            with redirect_stdout(buf):
                self.t.cmd_list(ns(dir=str(d), status=None, kind=None,
                                   priority=None, parent=None, label=None,
                                   sort="created", flat=True))
            out = buf.getvalue()
            pos_older = out.index("Older")
            pos_newer = out.index("Newer")
            self.assertLess(pos_older, pos_newer)


if __name__ == "__main__":
    unittest.main()
