import json
import unittest
from io import StringIO
from contextlib import redirect_stdout
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


def row(iid, *, status="done", closed=None, kind="task", parent=None,
        resolution=None, component=None, title=None):
    d = {"id": iid, "slug": f"i{iid}", "title": title or f"I{iid}", "kind": kind,
         "status": status, "priority": "medium"}
    if parent is not None:
        d["parent"] = parent
    if closed is not None:
        d["closed"] = closed
    if resolution is not None:
        d["resolution"] = resolution
    if component is not None:
        d["component"] = component
    return json.dumps(d, ensure_ascii=False)


class TestParseSince(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_accepts_bare_date(self):
        self.assertEqual(self.t.parse_since("2026-06-10"), "2026-06-10")

    def test_accepts_full_timestamp(self):
        self.assertEqual(self.t.parse_since("2026-06-10T14:00:00Z"), "2026-06-10T14:00:00Z")

    def test_rejects_garbage(self):
        for bad in ("june", "2026/06/10", "2026-6-10", "2026-06-10T14:00Z", ""):
            with self.subTest(bad=bad), self.assertRaises(SystemExit):
                self.t.parse_since(bad)


class TestSelectShipped(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def load(self, tmp, rows_json):
        d = make_tracker(tmp, {})
        (d / "index.jsonl").write_text("".join(r + "\n" for r in rows_json))
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return ctx, self.t.load_index(ctx)

    def ids(self, ctx, rows, since):
        return sorted(r.id for r in self.t.select_shipped(ctx.cfg, rows, since))

    def test_selection_matrix(self):
        with TemporaryDirectory() as tmp:
            ctx, rows = self.load(tmp, [
                row(1, closed="2026-06-11T10:00:00Z"),                       # in: terminal, after
                row(2, closed="2026-06-09T10:00:00Z"),                       # out: closed before since
                row(3, status="ongoing"),                                    # out: not terminal (no closed)
                row(4, closed="2026-06-11T10:00:00Z", resolution="wontfix"), # out: resolution
                row(5, closed="2026-06-12T10:00:00Z", kind="epic"),          # in: epics included
                row(6, closed="2026-06-12T10:00:00Z", kind="bug"),           # in: bugs included
            ])
            self.assertEqual(self.ids(ctx, rows, "2026-06-10"), [1, 5, 6])

    def test_bare_date_includes_same_day_timestamp(self):
        with TemporaryDirectory() as tmp:
            ctx, rows = self.load(tmp, [row(1, closed="2026-06-10T08:00:00Z")])
            self.assertEqual(self.ids(ctx, rows, "2026-06-10"), [1])

    def test_exact_timestamp_boundary_is_inclusive(self):
        with TemporaryDirectory() as tmp:
            ctx, rows = self.load(tmp, [row(1, closed="2026-06-10T08:00:00Z")])
            self.assertEqual(self.ids(ctx, rows, "2026-06-10T08:00:00Z"), [1])

    def test_legacy_day_only_closed_handled(self):
        with TemporaryDirectory() as tmp:
            ctx, rows = self.load(tmp, [row(1, closed="2026-06-11")])
            self.assertEqual(self.ids(ctx, rows, "2026-06-10"), [1])
            self.assertEqual(self.ids(ctx, rows, "2026-06-12"), [])


if __name__ == "__main__":
    unittest.main()
