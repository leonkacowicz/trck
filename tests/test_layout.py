"""The flat items/ layout: status lives only in index.jsonl, never in the path."""
import io
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestFlatLayout(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title="Item", **over):
        a = dict(dir=str(d), title=title, priority="high", kind=None, parent=None,
                 points=None, depends=None, spec=None, slug=None, pr=None)
        a.update(over)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(ns(**a))
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def test_new_writes_into_items_dir(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            files = sorted(p.name for p in (d / "items").glob("*.md"))
            self.assertEqual(len(files), 1)
            self.assertTrue(files[0].endswith("-alpha.md"))
            self.assertFalse((d / "backlog").exists())

    def test_status_change_does_not_move_the_file(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            before = sorted(p.name for p in (d / "items").glob("*.md"))
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_mv(ns(dir=str(d), id=iid, status="done",
                                 resolution=None, pr=None))
            after = sorted(p.name for p in (d / "items").glob("*.md"))
            self.assertEqual(before, after)
            self.assertFalse((d / "done").exists())
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            row = self.t.get_row(self.t.load_index(ctx), iid)
            self.assertEqual(row.status, "done")

    def test_rel_link_points_into_items_dir(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            row = self.t.get_row(self.t.load_index(ctx), iid)
            self.assertEqual(self.t.rel_link(row), f"items/{self.t.filename(row)}")
            self.assertIn(f"(items/{self.t.filename(row)})",
                          (d / "SUMMARY.md").read_text())

    def test_slug_change_still_renames_within_items(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_set(ns(dir=str(d), id=iid, slug="renamed", title=None,
                                  priority=None, points=None, parent=None, spec=None,
                                  pr=None, kind=None, field=None, unset=None, auto=False))
            files = sorted(p.name for p in (d / "items").glob("*.md"))
            self.assertEqual(files, [f"{iid}-renamed.md"])

    def test_move_issue_dies_when_the_body_file_is_missing(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            rows = self.t.load_index(ctx)
            row = self.t.get_row(rows, iid)
            self.t.issue_path(ctx, row).unlink()
            with self.assertRaises(SystemExit):
                self.t.move_issue(ctx, row, "done")

    def test_move_issue_stamps_dates_without_touching_the_file(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            row = self.t.get_row(self.t.load_index(ctx), iid)
            path = self.t.issue_path(ctx, row)
            mtime = path.stat().st_mtime_ns
            self.t.move_issue(ctx, row, "done")
            self.assertEqual(row.status, "done")
            self.assertIsNotNone(row.started)
            self.assertIsNotNone(row.closed)
            self.assertEqual(path.stat().st_mtime_ns, mtime)  # body untouched

    def test_scan_files_maps_id_to_slug_and_filename(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            found = self.t.scan_files(ctx)
            self.assertEqual(found[iid], ("alpha", f"{iid}-alpha.md"))


class TestLegacyLayoutGuard(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def legacy(self, tmp):
        """A tracker laid out the old way: one issue file under backlog/."""
        d = make_tracker(tmp, {})
        row = self.t.Issue(id="abc1234", slug="alpha", title="Alpha", kind="task",
                           status="backlog", priority="high")
        ctx = self.t.Ctx(d, self.t.load_config(d))
        old = d / "backlog"
        old.mkdir(parents=True, exist_ok=True)
        (old / self.t.filename(row)).write_text("# Alpha\n")
        self.t.save_index(ctx, [row])
        return d

    def test_detect_legacy_layout_finds_status_folder_files(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp)
            cfg = self.t.load_config(d)
            stale = self.t.detect_legacy_layout(cfg, d)
            self.assertEqual([p.name for p in stale], ["abc1234-alpha.md"])

    def test_detect_legacy_layout_is_empty_for_a_flat_tracker(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            (d / "items").mkdir()
            (d / "items" / "abc1234-alpha.md").write_text("# Alpha\n")
            self.assertEqual(self.t.detect_legacy_layout(self.t.load_config(d), d), [])

    def test_detect_legacy_layout_ignores_non_issue_markdown(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            (d / "done").mkdir()
            (d / "done" / "NOTES.md").write_text("scratch\n")
            self.assertEqual(self.t.detect_legacy_layout(self.t.load_config(d), d), [])

    def test_a_status_named_items_is_legal_and_never_self_detects(self):
        """Statuses no longer name directories, so `items` is an ordinary status
        value. Detection must not mistake the body dir for that status's folder."""
        config = {"statuses": [{"name": "backlog", "role": "initial"},
                               {"name": "items", "role": "active"},
                               {"name": "done", "role": "terminal"}],
                  "aliases": {"start": "items", "done": "done"}}
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, config)
            (d / "items").mkdir()
            (d / "items" / "abc1234-alpha.md").write_text("# Alpha\n")
            self.assertEqual(self.t.detect_legacy_layout(self.t.load_config(d), d), [])

    def test_commands_refuse_a_legacy_layout_tracker(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp)
            with self.assertRaises(SystemExit):
                self.t.build_ctx_or_die(ns(dir=str(d)))

    def test_guard_can_be_bypassed_for_the_migration_verb(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp)
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)), guard_layout=False)
            self.assertEqual(ctx.dir, d)


class TestMigrateLayout(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def legacy(self, tmp, *specs):
        """Build a legacy-layout tracker. Each spec is (id, slug, status)."""
        d = make_tracker(tmp, {})
        ctx = self.t.Ctx(d, self.t.load_config(d))
        rows = []
        for iid, slug, status in specs:
            row = self.t.Issue(id=iid, slug=slug, title=slug.title(), kind="task",
                               status=status, priority="high")
            folder = d / status
            folder.mkdir(parents=True, exist_ok=True)
            (folder / self.t.filename(row)).write_text(f"# {slug.title()}\n")
            rows.append(row)
        self.t.save_index(ctx, rows)
        return d

    def cap(self, fn, args):
        buf = io.StringIO()
        with redirect_stdout(buf):
            fn(args)
        return buf.getvalue()

    def test_migrate_moves_every_file_into_items(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"),
                                 ("bcd2345", "beta", "done"))
            self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))
            names = sorted(p.name for p in (d / "items").glob("*.md"))
            self.assertEqual(names, ["abc1234-alpha.md", "bcd2345-beta.md"])
            self.assertFalse((d / "backlog").exists())
            self.assertFalse((d / "done").exists())

    def test_migrate_preserves_status_from_the_index(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"),
                                 ("bcd2345", "beta", "done"))
            self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            by_id = {r.id: r.status for r in self.t.load_index(ctx)}
            self.assertEqual(by_id, {"abc1234": "backlog", "bcd2345": "done"})

    def test_check_passes_after_migration(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"))
            self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))
            out = self.cap(self.t.cmd_check, ns(dir=str(d)))
            self.assertIn("OK", out)

    def test_migrate_is_idempotent(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"))
            self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))
            out = self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))
            self.assertIn("nothing to migrate", out)

    def test_dry_run_writes_nothing(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"))
            out = self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=True))
            self.assertIn("abc1234-alpha.md", out)
            self.assertTrue((d / "backlog" / "abc1234-alpha.md").is_file())
            self.assertFalse((d / "items").exists())

    def test_migrate_dies_when_folder_and_index_status_disagree(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"))
            ctx = self.t.Ctx(d, self.t.load_config(d))
            rows = self.t.load_index(ctx)
            rows[0].status = "done"            # index says done, file sits in backlog/
            self.t.save_index(ctx, rows)
            with self.assertRaises(SystemExit):
                self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))
            self.assertTrue((d / "backlog" / "abc1234-alpha.md").is_file())

    def test_migrate_dies_on_a_destination_collision(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"))
            (d / "items").mkdir()
            (d / "items" / "abc1234-alpha.md").write_text("# squatter\n")
            with self.assertRaises(SystemExit):
                self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))

    def test_migrate_leaves_non_issue_files_in_place(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"))
            (d / "backlog" / "NOTES.md").write_text("scratch\n")
            self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))
            self.assertTrue((d / "backlog" / "NOTES.md").is_file())  # folder kept
            self.assertTrue((d / "items" / "abc1234-alpha.md").is_file())
