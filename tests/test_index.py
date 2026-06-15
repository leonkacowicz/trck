import json
import unittest
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker


class TestIndexIO(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def ctx(self, tmp):
        d = make_tracker(tmp, {})
        return self.t.Ctx(d, self.t.load_config(d))

    def issue(self, **over):
        fields = dict(id=1, slug="a", title="A", kind="task",
                      status="backlog", priority="high")
        fields.update(over)
        return self.t.Issue(**fields)

    def test_roundtrip_preserves_unknown_keys(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = self.issue(labels=["x"], extra={"zeta": 9})
            self.t.save_index(ctx, [row])
            back = self.t.load_index(ctx)
            self.assertEqual(back[0].labels, ["x"])
            self.assertEqual(back[0].extra["zeta"], 9)

    def test_known_keys_come_first_in_canonical_order(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = self.issue(id=7, title="T", slug="t", priority="low",
                             extra={"zeta": 1})
            self.t.save_index(ctx, [row])
            line = (ctx.dir / "index.jsonl").read_text().strip()
            keys = list(json.loads(line).keys())
            self.assertEqual(keys[:3], ["id", "slug", "title"])
            self.assertEqual(keys[-1], "zeta")  # unknown appended last

    def test_depends_on_defaults_to_list(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            self.t.save_index(ctx, [self.issue()])
            self.assertEqual(self.t.load_index(ctx)[0].depends_on, [])

    def test_strips_known_fields_equal_to_default(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = self.issue(created="2026-06-05")  # all optionals at default
            self.t.save_index(ctx, [row])
            obj = json.loads((ctx.dir / "index.jsonl").read_text().strip())
            for stripped in ("parent", "labels", "depends_on", "spec",
                             "started", "closed", "resolution"):
                self.assertNotIn(stripped, obj)
            self.assertEqual(list(obj.keys()),
                             ["id", "slug", "title", "kind", "status",
                              "priority", "created"])

    def test_keeps_non_default_known_fields_in_canon_order(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = self.issue(id=2, slug="b", title="B", status="done", priority="low",
                             parent=1, labels=["m1"], depends_on=[1],
                             created="2026-06-05", resolution="fixed")
            self.t.save_index(ctx, [row])
            obj = json.loads((ctx.dir / "index.jsonl").read_text().strip())
            self.assertEqual(obj["parent"], "1")
            self.assertEqual(obj["labels"], ["m1"])
            self.assertEqual(obj["depends_on"], ["1"])
            self.assertEqual(obj["resolution"], "fixed")
            self.assertNotIn("spec", obj)  # still default -> stripped
            self.assertEqual(list(obj.keys()),
                             ["id", "slug", "title", "kind", "status",
                              "priority", "parent", "labels", "depends_on",
                              "created", "resolution"])

    def test_custom_field_kept_even_when_empty(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = self.issue(extra={"custom_null": None, "custom_empty": [],
                                    "custom_str": ""})
            self.t.save_index(ctx, [row])
            obj = json.loads((ctx.dir / "index.jsonl").read_text().strip())
            self.assertIn("custom_null", obj)
            self.assertIsNone(obj["custom_null"])
            self.assertEqual(obj["custom_empty"], [])
            self.assertEqual(obj["custom_str"], "")

    def test_save_is_idempotent_byte_identical(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = self.issue(labels=["m1"], extra={"vendor": None})
            self.t.save_index(ctx, [row])
            first = (ctx.dir / "index.jsonl").read_bytes()
            self.t.save_index(ctx, self.t.load_index(ctx))
            self.assertEqual((ctx.dir / "index.jsonl").read_bytes(), first)

    def test_gen_id_avoids_existing_ids(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            self.t.save_index(ctx, [self.issue(id=3)])
            (ctx.dir / "backlog").mkdir()
            (ctx.dir / "backlog" / "010-x.md").write_text("# X")
            existing = {"3", "010", "10"}
            # gen_id must return an id not in the seen set
            for _ in range(20):
                new_id = self.t.gen_id(ctx)
                self.assertNotIn(new_id, existing)
                self.assertTrue(self.t.ID_RE.match(new_id))

    def test_get_row_missing_dies(self):
        with TemporaryDirectory() as tmp:
            self.ctx(tmp)
            with self.assertRaises(SystemExit):
                self.t.get_row([], 99)

    def test_load_index_dies_on_structurally_broken_row(self):
        # missing 'id' (the case that used to crash with a TypeError), a missing
        # required string, and a wrong-typed field all fail loudly at load.
        for row in (
            '{"slug":"a","title":"A","kind":"task","status":"backlog","priority":"high"}',
            '{"id":1,"title":"A","kind":"task","status":"backlog","priority":"high"}',
            '{"id":1,"slug":"a","title":"A","kind":"task","status":"backlog","priority":"high","points":"lots"}',
            '"not an object"',
        ):
            with TemporaryDirectory() as tmp:
                ctx = self.ctx(tmp)
                (ctx.dir / "index.jsonl").write_text(row + "\n")
                with self.assertRaises(SystemExit, msg=row):
                    self.t.load_index(ctx)
