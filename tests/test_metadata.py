import unittest
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestMetadata(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, **over):
        args = ns(dir=str(d), title=over.pop("title", "Item"), priority="high",
                  epic=over.pop("epic", False), parent=None, milestone=None,
                  depends=None, spec=None, slug=None)
        for k, v in over.items():
            setattr(args, k, v)
        self.t.cmd_new(args)

    def rows(self, d):
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return {r["id"]: r for r in self.t.load_index(ctx)}

    def set_args(self, d, iid, **over):
        a = ns(dir=str(d), id=iid, priority=None, parent=None, milestone=None,
               spec=None, kind=None, title=None)
        for k, v in over.items():
            setattr(a, k, v)
        return a

    def test_set_priority_and_milestone(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.t.cmd_set(self.set_args(d, 1, priority="low", milestone="M2"))
            r = self.rows(d)[1]
            self.assertEqual(r["priority"], "low")
            self.assertEqual(r["milestone"], "M2")

    def test_set_parent_requires_epic(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, title="Epic", epic=True)   # id 1, epic
            self.seed(d, title="Child")             # id 2
            self.t.cmd_set(self.set_args(d, 2, parent="1"))
            self.assertEqual(self.rows(d)[2]["parent"], 1)

    def test_set_milestone_none_clears(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.t.cmd_set(self.set_args(d, 1, milestone="M1"))
            self.t.cmd_set(self.set_args(d, 1, milestone="none"))
            self.assertIsNone(self.rows(d)[1]["milestone"])

    def test_dep_add_remove(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d); self.seed(d)
            self.t.cmd_dep(ns(dir=str(d), id=2, add=1, remove=None))
            self.assertEqual(self.rows(d)[2]["depends_on"], [1])
            self.t.cmd_dep(ns(dir=str(d), id=2, add=None, remove=1))
            self.assertEqual(self.rows(d)[2]["depends_on"], [])

    def test_rename_changes_title_slug_and_file(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, title="Old Name")
            self.t.cmd_rename(ns(dir=str(d), id=1, title="Brand New", slug=None))
            r = self.rows(d)[1]
            self.assertEqual(r["title"], "Brand New")
            self.assertEqual(r["slug"], "brand-new")
            self.assertTrue((d / "backlog" / "001-brand-new.md").exists())
            self.assertIn("# Brand New", (d / "backlog" / "001-brand-new.md").read_text())
