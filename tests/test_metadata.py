import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestMetadata(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def raw_rows(self, d):
        """Parse index.jsonl as written on disk (no default re-hydration)."""
        out = {}
        for line in (Path(d) / "index.jsonl").read_text().splitlines():
            if line.strip():
                obj = json.loads(line)
                out[obj["id"]] = obj
        return out

    def seed(self, d, **over):
        args = ns(dir=str(d), title=over.pop("title", "Item"), priority="high",
                  kind=over.pop("kind", None), parent=None,
                  depends=None, spec=None, slug=None)
        for k, v in over.items():
            setattr(args, k, v)
        self.t.cmd_new(args)

    def rows(self, d):
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return {r.id: r for r in self.t.load_index(ctx)}

    def set_args(self, d, iid, **over):
        a = ns(dir=str(d), id=iid, priority=None, parent=None,
               spec=None, kind=None, title=None)
        for k, v in over.items():
            setattr(a, k, v)
        return a

    def test_new_kind_defaults_to_first_configured(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.assertEqual(self.rows(d)[1].kind, "task")

    def test_new_kind_sets_configured_kind(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, kind="epic")
            self.assertEqual(self.rows(d)[1].kind, "epic")

    def test_new_rejects_unconfigured_kind(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            with self.assertRaises(SystemExit):
                self.seed(d, kind="bogus")

    def test_set_priority(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.t.cmd_set(self.set_args(d, 1, priority="low"))
            self.assertEqual(self.rows(d)[1].priority, "low")

    def test_set_parent_to_any_issue(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, title="Parent task")   # id 1, a plain task (not an epic)
            self.seed(d, title="Child")         # id 2
            self.t.cmd_set(self.set_args(d, 2, parent="1"))
            self.assertEqual(self.rows(d)[2].parent, 1)

    def test_set_parent_must_exist(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, title="Only")          # id 1
            with self.assertRaises(SystemExit):
                self.t.cmd_set(self.set_args(d, 1, parent="99"))

    def test_dep_add_remove(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d); self.seed(d)
            self.t.cmd_dep(ns(dir=str(d), id=2, add=1, remove=None))
            self.assertEqual(self.rows(d)[2].depends_on, [1])
            self.t.cmd_dep(ns(dir=str(d), id=2, add=None, remove=1))
            self.assertEqual(self.rows(d)[2].depends_on, [])

    def test_new_points_defaults_to_one(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.assertEqual(self.rows(d)[1].points, 1)

    def test_new_points_default_is_stripped_from_index(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.assertNotIn("points", self.raw_rows(d)[1])  # default 1 -> noise

    def test_new_points_explicit_is_stored(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, points=5)
            self.assertEqual(self.rows(d)[1].points, 5)
            self.assertEqual(self.raw_rows(d)[1]["points"], 5)

    def test_new_points_negative_rejected(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            with self.assertRaises(SystemExit):
                self.seed(d, points=-1)

    def test_set_points_updates_leaf(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.t.cmd_set(self.set_args(d, 1, points=3))
            self.assertEqual(self.rows(d)[1].points, 3)

    def test_set_points_negative_rejected(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            with self.assertRaises(SystemExit):
                self.t.cmd_set(self.set_args(d, 1, points=-2))

    def test_set_points_on_parent_is_error(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, title="Parent")   # id 1
            self.seed(d, title="Child")    # id 2
            self.t.cmd_set(self.set_args(d, 2, parent="1"))  # 1 now has a child
            with self.assertRaises(SystemExit):
                self.t.cmd_set(self.set_args(d, 1, points=4))

    def test_points_stripped_from_index_when_issue_gains_child(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, title="Parent", points=7)   # id 1, a leaf weighing 7
            self.seed(d, title="Child")              # id 2
            self.assertEqual(self.raw_rows(d)[1]["points"], 7)   # stored while a leaf
            self.t.cmd_set(self.set_args(d, 2, parent="1"))      # 1 becomes a parent
            self.assertNotIn("points", self.raw_rows(d)[1])      # derived now -> deleted

    def test_former_parent_returns_to_default_points(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, title="Parent", points=7)   # id 1
            self.seed(d, title="Child")              # id 2
            self.t.cmd_set(self.set_args(d, 2, parent="1"))    # 1 is a parent
            self.t.cmd_set(self.set_args(d, 2, parent="none")) # 1 is a leaf again
            self.assertEqual(self.rows(d)[1].points, 1)     # default; old 7 not restored

    def test_rename_changes_title_slug_and_file(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, title="Old Name")
            self.t.cmd_rename(ns(dir=str(d), id=1, title="Brand New", slug=None))
            r = self.rows(d)[1]
            self.assertEqual(r.title, "Brand New")
            self.assertEqual(r.slug, "brand-new")
            self.assertTrue((d / "backlog" / "001-brand-new.md").exists())
            self.assertIn("# Brand New", (d / "backlog" / "001-brand-new.md").read_text())
