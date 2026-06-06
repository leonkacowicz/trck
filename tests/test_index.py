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

    def test_strips_known_fields_equal_to_default(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = {"id": 1, "slug": "a", "title": "A", "kind": "task",
                   "status": "backlog", "priority": "high", "parent": None,
                   "milestone": None, "depends_on": [], "spec": None,
                   "created": "2026-06-05", "started": None, "closed": None,
                   "resolution": None}
            self.t.save_index(ctx, [row])
            obj = json.loads((ctx.dir / "index.jsonl").read_text().strip())
            for stripped in ("parent", "milestone", "depends_on", "spec",
                             "started", "closed", "resolution"):
                self.assertNotIn(stripped, obj)
            self.assertEqual(list(obj.keys()),
                             ["id", "slug", "title", "kind", "status",
                              "priority", "created"])

    def test_keeps_non_default_known_fields_in_canon_order(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = {"id": 2, "slug": "b", "title": "B", "kind": "task",
                   "status": "done", "priority": "low", "parent": 1,
                   "milestone": "m1", "depends_on": [1], "spec": None,
                   "created": "2026-06-05", "started": None, "closed": None,
                   "resolution": "fixed"}
            self.t.save_index(ctx, [row])
            obj = json.loads((ctx.dir / "index.jsonl").read_text().strip())
            self.assertEqual(obj["parent"], 1)
            self.assertEqual(obj["milestone"], "m1")
            self.assertEqual(obj["depends_on"], [1])
            self.assertEqual(obj["resolution"], "fixed")
            self.assertNotIn("spec", obj)  # still default -> stripped
            self.assertEqual(list(obj.keys()),
                             ["id", "slug", "title", "kind", "status",
                              "priority", "parent", "milestone", "depends_on",
                              "created", "resolution"])

    def test_custom_field_kept_even_when_empty(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = {"id": 1, "slug": "a", "title": "A", "kind": "task",
                   "status": "backlog", "priority": "high",
                   "custom_null": None, "custom_empty": [], "custom_str": ""}
            self.t.save_index(ctx, [row])
            obj = json.loads((ctx.dir / "index.jsonl").read_text().strip())
            self.assertIn("custom_null", obj)
            self.assertIsNone(obj["custom_null"])
            self.assertEqual(obj["custom_empty"], [])
            self.assertEqual(obj["custom_str"], "")

    def test_save_is_idempotent_byte_identical(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = {"id": 1, "slug": "a", "title": "A", "kind": "task",
                   "status": "backlog", "priority": "high", "milestone": "m1",
                   "extra": None}
            self.t.save_index(ctx, [row])
            first = (ctx.dir / "index.jsonl").read_bytes()
            self.t.save_index(ctx, self.t.load_index(ctx))
            self.assertEqual((ctx.dir / "index.jsonl").read_bytes(), first)

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
