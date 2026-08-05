import io
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestMetadata(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, **over):
        args = ns(dir=str(d), title=over.pop("title", "Item"), priority="high",
                  parent=None,
                  depends=None, spec=None, slug=None)
        for k, v in over.items():
            setattr(args, k, v)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(args)
        prefix = Path(buf.getvalue().strip()).name.split("-")[0]
        return str(int(prefix)) if prefix.isdigit() else prefix

    def rows(self, d):
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return {r.id: r for r in self.t.load_index(ctx)}

    def set_args(self, d, iid, **over):
        a = ns(dir=str(d), id=iid, priority=None, parent=None,
               spec=None, title=None, slug=None)
        for k, v in over.items():
            setattr(a, k, v)
        return a

    def test_dep_add_allows_valid_dag(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            id2 = self.seed(d)
            id3 = self.seed(d)
            self.t.cmd_dep(ns(dir=str(d), id=id2, add=id1, remove=None))  # 2 -> 1
            self.t.cmd_dep(ns(dir=str(d), id=id3, add=id1, remove=None))  # 3 -> 1 (diamond base)
            self.t.cmd_dep(ns(dir=str(d), id=id3, add=id2, remove=None))  # 3 -> 2, still a DAG
            self.assertEqual(sorted(self.rows(d)[id3].depends_on), sorted([id1, id2]))

    def test_set_reparent_independent_subtree_still_allowed(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            p1 = self.seed(d, title="P1")
            p2 = self.seed(d, title="P2")
            self.t.cmd_set(self.set_args(d, p1, parent=p2))  # harmless reparent
            self.assertEqual(self.rows(d)[p1].parent, p2)

    def test_set_slug_moves_the_file(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, title="Old Name")
            self.t.cmd_set(self.set_args(d, id1, slug="renamed"))
            ctx = self.t.Ctx(d, self.t.load_config(d))
            r = self.rows(d)[id1]
            self.assertEqual(r.slug, "renamed")
            self.assertTrue(self.t.issue_path(ctx, r).exists())
            self.assertFalse(any(
                f.name.endswith("-old-name.md")
                for f in (d / "items").iterdir()
            ))

    def test_set_title_and_slug_together(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, title="Old Name")
            self.t.cmd_set(self.set_args(d, id1, title="Brand New", slug="brand-new"))
            ctx = self.t.Ctx(d, self.t.load_config(d))
            r = self.rows(d)[id1]
            self.assertEqual(r.title, "Brand New")
            self.assertEqual(r.slug, "brand-new")
            new_path = self.t.issue_path(ctx, r)
            self.assertTrue(new_path.exists())
            self.assertIn("# Brand New", new_path.read_text())
