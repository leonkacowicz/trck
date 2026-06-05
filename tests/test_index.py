import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker


class TestIndexIO(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def ctx(self, tmp):
        d = make_tracker(tmp, {})
        return self.t.Ctx(d, self.t.load_config(d))

    def test_roundtrip_preserves_unknown_keys(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = {"id": 1, "slug": "a", "title": "A", "kind": "task",
                   "status": "backlog", "priority": "high", "labels": ["x"],
                   "zeta": 9}
            self.t.save_index(ctx, [row])
            back = self.t.load_index(ctx)
            self.assertEqual(back[0]["labels"], ["x"])
            self.assertEqual(back[0]["zeta"], 9)

    def test_known_keys_come_first_in_canonical_order(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = {"zeta": 1, "id": 7, "title": "T", "slug": "t", "kind": "task",
                   "status": "backlog", "priority": "low"}
            self.t.save_index(ctx, [row])
            line = (ctx.dir / "index.jsonl").read_text().strip()
            keys = list(json.loads(line).keys())
            self.assertEqual(keys[:3], ["id", "slug", "title"])
            self.assertEqual(keys[-1], "zeta")  # unknown appended last

    def test_depends_on_defaults_to_list(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            self.t.save_index(ctx, [{"id": 1, "slug": "a", "title": "A",
                                     "kind": "task", "status": "backlog",
                                     "priority": "high"}])
            self.assertEqual(self.t.load_index(ctx)[0]["depends_on"], [])

    def test_next_id_counts_index_and_disk(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            self.t.save_index(ctx, [{"id": 3, "slug": "a", "title": "A",
                                     "kind": "task", "status": "backlog",
                                     "priority": "high"}])
            (ctx.dir / "backlog").mkdir()
            (ctx.dir / "backlog" / "010-x.md").write_text("# X")
            self.assertEqual(self.t.next_id(ctx), 11)

    def test_get_row_missing_dies(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            with self.assertRaises(SystemExit):
                self.t.get_row([], 99)
