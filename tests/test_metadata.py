import io
import json
import unittest
from contextlib import redirect_stdout
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
               spec=None, kind=None, title=None, slug=None)
        for k, v in over.items():
            setattr(a, k, v)
        return a

    def test_new_kind_defaults_to_first_configured(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            self.assertEqual(self.rows(d)[id1].kind, "task")

    def test_new_priority_defaults_to_medium(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, priority=None)
            self.assertEqual(self.rows(d)[id1].priority, "medium")

    def test_new_kind_sets_configured_kind(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, kind="epic")
            self.assertEqual(self.rows(d)[id1].kind, "epic")

    def test_new_rejects_unconfigured_kind(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            with self.assertRaises(SystemExit):
                self.seed(d, kind="bogus")

    def test_set_priority(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            self.t.cmd_set(self.set_args(d, id1, priority="low"))
            self.assertEqual(self.rows(d)[id1].priority, "low")

    def test_set_parent_to_any_issue(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, title="Parent task")
            id2 = self.seed(d, title="Child")
            self.t.cmd_set(self.set_args(d, id2, parent=id1))
            self.assertEqual(self.rows(d)[id2].parent, id1)

    def test_set_parent_must_exist(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, title="Only")
            with self.assertRaises(SystemExit):
                self.t.cmd_set(self.set_args(d, id1, parent="99"))

    def test_dep_add_remove(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            id2 = self.seed(d)
            self.t.cmd_dep(ns(dir=str(d), id=id2, add=id1, remove=None))
            self.assertEqual(self.rows(d)[id2].depends_on, [id1])
            self.t.cmd_dep(ns(dir=str(d), id=id2, add=None, remove=id1))
            self.assertEqual(self.rows(d)[id2].depends_on, [])

    def test_dep_add_rejects_self_edge(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            with self.assertRaises(SystemExit):
                self.t.cmd_dep(ns(dir=str(d), id=id1, add=id1, remove=None))
            self.assertEqual(self.rows(d)[id1].depends_on, [])

    def test_dep_add_rejects_two_node_cycle(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            id2 = self.seed(d)
            self.t.cmd_dep(ns(dir=str(d), id=id1, add=id2, remove=None))  # 1 -> 2
            with self.assertRaises(SystemExit):
                self.t.cmd_dep(ns(dir=str(d), id=id2, add=id1, remove=None))  # 2 -> 1 closes cycle
            self.assertEqual(self.rows(d)[id2].depends_on, [])  # not written

    def test_dep_add_rejects_longer_cycle(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            id2 = self.seed(d)
            id3 = self.seed(d)
            self.t.cmd_dep(ns(dir=str(d), id=id1, add=id2, remove=None))  # 1 -> 2
            self.t.cmd_dep(ns(dir=str(d), id=id2, add=id3, remove=None))  # 2 -> 3
            with self.assertRaises(SystemExit):
                self.t.cmd_dep(ns(dir=str(d), id=id3, add=id1, remove=None))  # 3 -> 1 closes cycle
            self.assertEqual(self.rows(d)[id3].depends_on, [])  # not written

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

    def test_dep_add_rejects_child_to_ancestor(self):
        # child cannot depend on its own parent/ancestor (disjoint-subtree invariant).
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            parent = self.seed(d, title="Epic")
            child = self.seed(d, parent=parent)
            with self.assertRaises(SystemExit):
                self.t.cmd_dep(ns(dir=str(d), id=child, add=parent, remove=None))
            self.assertEqual(self.rows(d)[child].depends_on, [])

    def test_dep_add_rejects_ancestor_to_descendant(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            parent = self.seed(d, title="Epic")
            child = self.seed(d, parent=parent)
            with self.assertRaises(SystemExit):
                self.t.cmd_dep(ns(dir=str(d), id=parent, add=child, remove=None))
            self.assertEqual(self.rows(d)[parent].depends_on, [])

    def test_dep_add_allows_siblings_under_common_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            parent = self.seed(d, title="Epic")
            a = self.seed(d, parent=parent)
            b = self.seed(d, parent=parent)
            self.t.cmd_dep(ns(dir=str(d), id=a, add=b, remove=None))  # sibling dep is fine
            self.assertEqual(self.rows(d)[a].depends_on, [b])

    def test_dep_add_rejects_lifted_cousin_cycle(self):
        # P2 -> P1; then C1(P1) -> C2(P2) is an effective (inherited) deadlock.
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            p1 = self.seed(d, title="P1")
            p2 = self.seed(d, title="P2")
            c1 = self.seed(d, parent=p1)
            c2 = self.seed(d, parent=p2)
            self.t.cmd_dep(ns(dir=str(d), id=p2, add=p1, remove=None))  # P2 -> P1
            with self.assertRaises(SystemExit):
                self.t.cmd_dep(ns(dir=str(d), id=c1, add=c2, remove=None))  # C1 -> C2 deadlock
            self.assertEqual(self.rows(d)[c1].depends_on, [])

    def test_new_with_dep_on_ancestor_rejected(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            parent = self.seed(d, title="Epic")
            with self.assertRaises(SystemExit):
                self.seed(d, parent=parent, depends=parent)  # new child depending on parent

    def test_set_reparent_creating_effective_cycle_rejected(self):
        # P2 depends on P1; re-parenting P1 under P2 forms an effective cycle.
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            p1 = self.seed(d, title="P1")
            p2 = self.seed(d, title="P2")
            self.t.cmd_dep(ns(dir=str(d), id=p2, add=p1, remove=None))  # P2 -> P1
            with self.assertRaises(SystemExit):
                self.t.cmd_set(self.set_args(d, p1, parent=p2))         # P1 under P2 -> cycle
            self.assertIsNone(self.rows(d)[p1].parent)                  # not written

    def test_set_reparent_independent_subtree_still_allowed(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            p1 = self.seed(d, title="P1")
            p2 = self.seed(d, title="P2")
            self.t.cmd_set(self.set_args(d, p1, parent=p2))  # harmless reparent
            self.assertEqual(self.rows(d)[p1].parent, p2)

    def test_dep_remove_unknown_id_dies(self):
        """--remove now resolves via resolve_ref, so a non-existent token is a hard error."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            with self.assertRaises(SystemExit):
                self.t.cmd_dep(ns(dir=str(d), id=id1, add=None, remove="zzzzzzz"))

    def test_new_points_defaults_to_one(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            self.assertEqual(self.rows(d)[id1].points, 1)

    def test_new_points_default_is_stripped_from_index(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            self.assertNotIn("points", self.raw_rows(d)[id1])  # default 1 -> noise

    def test_new_points_explicit_is_stored(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, points=5)
            self.assertEqual(self.rows(d)[id1].points, 5)
            self.assertEqual(self.raw_rows(d)[id1]["points"], 5)

    def test_new_points_negative_rejected(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            with self.assertRaises(SystemExit):
                self.seed(d, points=-1)

    def test_set_points_updates_leaf(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            self.t.cmd_set(self.set_args(d, id1, points=3))
            self.assertEqual(self.rows(d)[id1].points, 3)

    def test_set_points_negative_rejected(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            with self.assertRaises(SystemExit):
                self.t.cmd_set(self.set_args(d, id1, points=-2))

    def test_set_points_on_parent_is_error(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, title="Parent")
            id2 = self.seed(d, title="Child")
            self.t.cmd_set(self.set_args(d, id2, parent=id1))  # id1 now has a child
            with self.assertRaises(SystemExit):
                self.t.cmd_set(self.set_args(d, id1, points=4))

    def test_points_stripped_from_index_when_issue_gains_child(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, title="Parent", points=7)   # a leaf weighing 7
            id2 = self.seed(d, title="Child")
            self.assertEqual(self.raw_rows(d)[id1]["points"], 7)   # stored while a leaf
            self.t.cmd_set(self.set_args(d, id2, parent=id1))     # id1 becomes a parent
            self.assertNotIn("points", self.raw_rows(d)[id1])      # derived now -> deleted

    def test_former_parent_returns_to_default_points(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, title="Parent", points=7)
            id2 = self.seed(d, title="Child")
            self.t.cmd_set(self.set_args(d, id2, parent=id1))    # id1 is a parent
            self.t.cmd_set(self.set_args(d, id2, parent="none")) # id1 is a leaf again
            self.assertEqual(self.rows(d)[id1].points, 1)     # default; old 7 not restored

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

    def test_set_slug_rejects_invalid_slug(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, title="Old Name")
            with self.assertRaises(SystemExit):
                self.t.cmd_set(self.set_args(d, id1, slug="Not A Slug"))

    def test_set_title_rewrites_the_h1(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, title="Old Name")
            ctx = self.t.Ctx(d, self.t.load_config(d))
            # capture path before the title change (slug stays the same)
            old_path = self.t.issue_path(ctx, self.rows(d)[id1])
            self.t.cmd_set(self.set_args(d, id1, title="Brand New"))
            r = self.rows(d)[id1]
            self.assertEqual(r.title, "Brand New")
            self.assertIn("# Brand New", old_path.read_text())

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
