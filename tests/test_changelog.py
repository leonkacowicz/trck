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


if __name__ == "__main__":
    unittest.main()
