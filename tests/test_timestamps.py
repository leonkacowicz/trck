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


if __name__ == "__main__":
    unittest.main()
