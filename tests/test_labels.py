import io
import json
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestLabels(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    # -- helpers -------------------------------------------------------------
    def seed(self, d, **over):
        args = ns(dir=str(d), title=over.pop("title", "Item"), priority="high",
                  kind=over.pop("kind", None), parent=None, depends=None,
                  spec=None, slug=None, points=None)
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

    def raw_lines(self, d):
        return (Path(d) / "index.jsonl").read_text()

    def label(self, d, iid, add=None, remove=None):
        self.t.cmd_label(ns(dir=str(d), id=iid, add=add, remove=remove))

    # -- the label verb ------------------------------------------------------
    def test_label_add_records_label(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            self.label(d, id1, add=["backend"])
            self.assertEqual(self.rows(d)[id1].labels, ["backend"])

    def test_label_add_multiple_dedups_and_sorts(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            self.label(d, id1, add=["beta", "alpha", "beta"])
            self.assertEqual(self.rows(d)[id1].labels, ["alpha", "beta"])

    def test_label_remove_drops_label(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            self.label(d, id1, add=["x", "y"])
            self.label(d, id1, remove=["x"])
            self.assertEqual(self.rows(d)[id1].labels, ["y"])

    def test_label_remove_missing_is_noop(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            self.label(d, id1, remove=["nope"])  # should not raise
            self.assertEqual(self.rows(d)[id1].labels, [])

    def test_default_labels_stripped_from_index(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.assertNotIn('"labels"', self.raw_lines(d))  # empty list is default

    # -- list --label --------------------------------------------------------
    def test_list_filters_by_label(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, title="Has label")
            self.seed(d, title="No label")
            self.label(d, id1, add=["urgent"])
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_list(ns(dir=str(d), status=None, kind=None,
                                   priority=None, parent=None, label="urgent"))
            out = buf.getvalue()
            self.assertIn("Has label", out)
            self.assertNotIn("No label", out)

    def test_list_shows_labels_as_tags(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            self.label(d, id1, add=["backend"])
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_list(ns(dir=str(d), status=None, kind=None,
                                   priority=None, parent=None, label=None))
            self.assertIn("backend", buf.getvalue())

    # -- show ----------------------------------------------------------------
    def test_show_displays_labels(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            self.label(d, id1, add=["docs"])
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_show(ns(dir=str(d), id=id1, json=False))
            out = buf.getvalue()
            self.assertIn("labels", out)
            self.assertIn("docs", out)

    # -- summary -------------------------------------------------------------
    def test_summary_shows_child_labels(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            ctx = self.t.Ctx(d, self.t.load_config(d))
            epic = self.t.Issue(id=1, slug="e", title="Epic", kind="epic",
                                status="ongoing", priority="high")
            kid = self.t.Issue(id=2, slug="k", title="Kid", kind="task",
                               status="ongoing", priority="high", parent=1,
                               labels=["frontend"])
            for r in (epic, kid):
                p = self.t.issue_path(ctx, r)
                p.parent.mkdir(parents=True, exist_ok=True)
                p.write_text("# x\n")
            self.t.save_index(ctx, [epic, kid])
            text = self.t.generate_summary(ctx)
            self.assertIn("frontend", text)

    def test_summary_shows_standalone_labels(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            ctx = self.t.Ctx(d, self.t.load_config(d))
            r = self.t.Issue(id=1, slug="a", title="A", kind="task",
                             status="backlog", priority="high", labels=["chore"])
            p = self.t.issue_path(ctx, r)
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text("# x\n")
            self.t.save_index(ctx, [r])
            text = self.t.generate_summary(ctx)
            self.assertIn("chore", text)

    # -- migration -----------------------------------------------------------
    def test_load_migrates_milestone_to_label(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            ctx = self.t.Ctx(d, self.t.load_config(d))
            legacy = ('{"id": 1, "slug": "a", "title": "A", "kind": "task", '
                      '"status": "backlog", "priority": "high", '
                      '"milestone": "v1.0"}\n')
            (Path(d) / "index.jsonl").write_text(legacy)
            rows = self.t.load_index(ctx)
            self.assertEqual(rows[0].labels, ["v1.0"])
            self.assertNotIn("milestone", rows[0].extra)

    def test_null_milestone_dropped_without_label(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            ctx = self.t.Ctx(d, self.t.load_config(d))
            legacy = ('{"id": 1, "slug": "a", "title": "A", "kind": "task", '
                      '"status": "backlog", "priority": "high", '
                      '"milestone": null}\n')
            (Path(d) / "index.jsonl").write_text(legacy)
            rows = self.t.load_index(ctx)
            self.assertEqual(rows[0].labels, [])
            self.assertNotIn("milestone", rows[0].extra)

    def test_normalize_persists_milestone_migration(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            ctx = self.t.Ctx(d, self.t.load_config(d))
            r = self.t.Issue(id=1, slug="a", title="A", kind="task",
                             status="backlog", priority="high")
            p = self.t.issue_path(ctx, r)
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text("# x\n")
            legacy = ('{"id": 1, "slug": "a", "title": "A", "kind": "task", '
                      '"status": "backlog", "priority": "high", '
                      '"milestone": "v1.0"}\n')
            (Path(d) / "index.jsonl").write_text(legacy)
            with redirect_stdout(io.StringIO()):
                self.t.cmd_normalize(ns(dir=str(d)))
            line = self.raw_lines(d)
            self.assertNotIn("milestone", line)
            self.assertIn("v1.0", line)
            self.assertEqual(self.rows(d)["1"].labels, ["v1.0"])

    # -- validation ----------------------------------------------------------
    def test_non_string_labels_fail_loud_at_load(self):
        # a non-string label is a wrong-typed value -> load fails loud, rather
        # than a soft validate error.
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            ctx = self.t.Ctx(d, self.t.load_config(d))
            r = self.t.Issue(id=1, slug="a", title="A", kind="task",
                             status="backlog", priority="high", labels=[5])
            p = self.t.issue_path(ctx, r)
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text("# x\n")
            self.t.save_index(ctx, [r])
            with self.assertRaises(SystemExit):
                self.t.validate(ctx)  # reloads the index -> from_dict rejects it


if __name__ == "__main__":
    unittest.main()
