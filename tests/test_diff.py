"""`trck diff` foundation: the VCS-agnostic source seam (#q9cq65c) and the
change model (#u8qaqwr).

Every test here builds snapshots from plain fixture files — no git repository,
no commits, no `subprocess`. That is the point of the seam: only the git
convenience layer (#wtmfdhr) needs a real repository fixture.
"""
import io
import json
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


def row(iid, **over):
    d = {"id": iid, "slug": f"i{iid}", "title": f"I{iid}", "kind": "task",
         "status": "backlog", "priority": "medium"}
    d.update(over)
    return d


def index_text(*rows):
    return "".join(json.dumps(r, ensure_ascii=False) + "\n" for r in rows)


class DiffTestCase(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.cfg = self.t.DEFAULT_CONFIG

    def snap(self, *rows, label="snap"):
        """A rows-only snapshot straight from fixture JSON."""
        return self.t.snapshot_from_text(index_text(*rows), label)

    def changes(self, old_rows, new_rows):
        d = self.t.diff_snapshots(self.cfg, self.snap(*old_rows), self.snap(*new_rows))
        return {c.id: c for c in d.changes}

    def tracker(self, tmp, *rows, bodies=None):
        """A real tracker dir on disk: trck.json, index.jsonl, and items/ bodies."""
        d = make_tracker(tmp, {})
        (d / "index.jsonl").write_text(index_text(*rows))
        items = d / "items"
        items.mkdir(exist_ok=True)
        for iid, text in (bodies or {}).items():
            (items / f"{iid}-i{iid}.md").write_text(text)
        return d


# --------------------------------------------------------------------------- #
# the change model (#u8qaqwr)
# --------------------------------------------------------------------------- #
class TestChangeClassification(DiffTestCase):
    def test_unchanged_snapshots_produce_no_changes(self):
        self.assertEqual(self.changes([row("aaa")], [row("aaa")]), {})

    def test_added_and_removed_rows(self):
        c = self.changes([row("aaa")], [row("bbb")])
        self.assertEqual(c["bbb"].kind, "added")
        self.assertEqual(c["aaa"].kind, "removed")
        self.assertIsNone(c["bbb"].old)
        self.assertIsNone(c["aaa"].new)

    def test_empty_old_snapshot_reads_as_all_added(self):
        c = self.changes([], [row("aaa"), row("bbb")])
        self.assertEqual({k: v.kind for k, v in c.items()},
                         {"aaa": "added", "bbb": "added"})

    def test_scalar_field_edit(self):
        c = self.changes([row("aaa")], [row("aaa", priority="high")])
        self.assertEqual(c["aaa"].kind, "modified")
        self.assertEqual([(f.name, f.old, f.new) for f in c["aaa"].fields],
                         [("priority", "medium", "high")])

    def test_multi_field_edit_reports_every_field(self):
        c = self.changes([row("aaa")],
                         [row("aaa", priority="high", points=5, parent="bbb")])
        self.assertEqual({f.name for f in c["aaa"].fields},
                         {"priority", "points", "parent"})

    def test_custom_fields_are_compared_too(self):
        c = self.changes([row("aaa", assignee="pat")], [row("aaa", assignee="sam")])
        self.assertEqual([(f.name, f.old, f.new) for f in c["aaa"].fields],
                         [("assignee", "pat", "sam")])

    def test_custom_field_appearing_and_disappearing(self):
        added = self.changes([row("aaa")], [row("aaa", assignee="sam")])
        self.assertEqual([(f.name, f.old, f.new) for f in added["aaa"].fields],
                         [("assignee", None, "sam")])
        gone = self.changes([row("aaa", assignee="sam")], [row("aaa")])
        self.assertEqual([(f.name, f.old, f.new) for f in gone["aaa"].fields],
                         [("assignee", "sam", None)])

    def test_set_fields_report_added_and_removed_members(self):
        c = self.changes([row("aaa", labels=["ui", "later"], depends_on=["bbb"])],
                         [row("aaa", labels=["ui", "perf"], depends_on=["bbb", "ccc"])])
        sets = {s.name: (s.added, s.removed) for s in c["aaa"].sets}
        self.assertEqual(sets["labels"], (["perf"], ["later"]))
        self.assertEqual(sets["depends_on"], (["ccc"], []))
        # set-valued fields never leak into the scalar list
        self.assertEqual([f.name for f in c["aaa"].fields], [])

    def test_reordering_a_set_field_is_not_a_change(self):
        self.assertEqual(self.changes([row("aaa", labels=["ui", "perf"])],
                                      [row("aaa", labels=["perf", "ui"])]), {})

    def test_timestamps_are_recorded_but_not_ordinary_field_edits(self):
        c = self.changes([row("aaa", status="backlog")],
                         [row("aaa", status="ongoing", started="2026-07-30T10:00:00Z")])
        self.assertEqual([f.name for f in c["aaa"].fields], ["status"])
        self.assertEqual(c["aaa"].timestamps,
                         {"started": (None, "2026-07-30T10:00:00Z")})

    def test_a_timestamp_only_edit_still_counts_as_a_change(self):
        c = self.changes([row("aaa")], [row("aaa", created="2026-07-30T10:00:00Z")])
        self.assertEqual(c["aaa"].kind, "modified")
        self.assertEqual(c["aaa"].fields, [])


class TestStatusDirection(DiffTestCase):
    def direction(self, old_status, new_status, cfg=None):
        d = self.t.diff_snapshots(cfg or self.cfg,
                                  self.snap(row("aaa", status=old_status)),
                                  self.snap(row("aaa", status=new_status)))
        return d.changes[0].direction

    def test_forward_move(self):
        self.assertEqual(self.direction("backlog", "ongoing"), "forward")
        self.assertEqual(self.direction("ongoing", "done"), "forward")

    def test_backward_move_is_distinguishable_from_a_start(self):
        # the whole point: a reopen must not render like a `backlog -> ongoing` start
        self.assertEqual(self.direction("done", "ongoing"), "backward")

    def test_no_status_change_has_no_direction(self):
        # a row that moved in some other way still reports no direction
        d = self.t.diff_snapshots(self.cfg, self.snap(row("aaa")),
                                  self.snap(row("aaa", priority="high")))
        self.assertIsNone(d.changes[0].direction)

    def test_unknown_status_is_lateral_not_a_crash(self):
        # an old snapshot may use a vocabulary this trck.json no longer has
        self.assertEqual(self.direction("archived", "done"), "lateral")
        self.assertEqual(self.direction("done", "archived"), "lateral")

    def test_direction_follows_the_vocabulary_order(self):
        """Forward is down the vocabulary, backward is up it — read from the status list
        rather than from any one status name, so the rule is the ordering itself."""
        self.assertEqual(self.direction("backlog", "in-review", self.cfg), "forward")
        self.assertEqual(self.direction("in-review", "backlog", self.cfg), "backward")
        self.assertEqual(self.direction("ongoing", "done", self.cfg), "forward")


class TestDiffResult(DiffTestCase):
    def test_diff_carries_both_snapshots(self):
        old, new = self.snap(row("aaa"), label="old"), self.snap(row("bbb"), label="new")
        d = self.t.diff_snapshots(self.cfg, old, new)
        self.assertIs(d.old, old)
        self.assertIs(d.new, new)

    def test_changes_are_ordered_by_id(self):
        d = self.t.diff_snapshots(self.cfg, self.snap(), self.snap(row("ccc"), row("aaa")))
        self.assertEqual([c.id for c in d.changes], ["aaa", "ccc"])

    def test_model_is_pure_and_knows_nothing_about_revisions(self):
        import inspect
        src = inspect.getsource(self.t.diff_snapshots)
        self.assertNotIn("subprocess", src)
        self.assertNotIn("git", src)


# --------------------------------------------------------------------------- #
# the source seam (#q9cq65c)
# --------------------------------------------------------------------------- #
class TestSnapshotSources(DiffTestCase):
    def test_snapshot_from_a_tracker_dir_yields_rows_and_bodies(self):
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, row("aaa"), bodies={"aaa": "# I aaa\n\nprose\n"})
            s = self.t.snapshot_from_dir(d, "fixture")
            self.assertEqual([r.id for r in s.rows], ["aaa"])
            self.assertTrue(s.has_bodies)
            self.assertEqual(s.body("aaa"), "# I aaa\n\nprose\n")

    def test_snapshot_from_an_index_file_has_rows_but_no_bodies(self):
        with TemporaryDirectory() as tmp:
            p = Path(tmp) / "old-index.jsonl"
            p.write_text(index_text(row("aaa")))
            s = self.t.resolve_source(str(p))
            self.assertEqual([r.id for r in s.rows], ["aaa"])
            self.assertFalse(s.has_bodies)
            self.assertIsNone(s.body("aaa"))

    def test_unavailable_bodies_are_distinct_from_an_empty_body(self):
        # #6xcseef depends on the difference: `None` means "this source cannot
        # supply bodies", which is not the same as "the body is empty".
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, row("aaa"), bodies={"aaa": ""})
            self.assertEqual(self.t.snapshot_from_dir(d, "fixture").body("aaa"), "")
            self.assertIsNone(self.t.snapshot_from_text(index_text(row("aaa")), "x").body("aaa"))

    def test_body_of_an_unknown_id_is_none(self):
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, row("aaa"), bodies={"aaa": "prose"})
            self.assertIsNone(self.t.snapshot_from_dir(d, "fixture").body("zzz"))

    def test_resolve_source_accepts_a_tracker_dir(self):
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, row("aaa"), bodies={"aaa": "prose"})
            s = self.t.resolve_source(str(d))
            self.assertEqual([r.id for r in s.rows], ["aaa"])
            self.assertTrue(s.has_bodies)

    def test_resolve_source_reads_stdin_for_a_dash(self):
        import sys
        old_stdin = sys.stdin
        sys.stdin = io.StringIO(index_text(row("aaa")))
        try:
            s = self.t.resolve_source("-")
        finally:
            sys.stdin = old_stdin
        self.assertEqual([r.id for r in s.rows], ["aaa"])
        self.assertFalse(s.has_bodies)
        self.assertEqual(s.label, "stdin")

    def test_snapshot_labels_default_to_the_source(self):
        with TemporaryDirectory() as tmp:
            p = Path(tmp) / "old-index.jsonl"
            p.write_text(index_text(row("aaa")))
            self.assertEqual(self.t.resolve_source(str(p)).label, "old-index.jsonl")

    def test_missing_source_fails_with_a_message_naming_the_path(self):
        with TemporaryDirectory() as tmp:
            missing = str(Path(tmp) / "nope.jsonl")
            buf = io.StringIO()
            with self.assertRaises(SystemExit), redirect_stdout(buf):
                self.t.resolve_source(missing)

    def test_malformed_source_fails_cleanly_naming_the_source(self):
        with TemporaryDirectory() as tmp:
            p = Path(tmp) / "broken.jsonl"
            p.write_text("{not json}\n")
            with self.assertRaises(SystemExit):
                self.t.resolve_source(str(p))

    def test_a_tracker_dir_without_an_index_is_an_empty_snapshot(self):
        # the tracker not existing on one side is not an error: everything is added
        with TemporaryDirectory() as tmp:
            d = Path(tmp) / "gone"
            d.mkdir()
            self.assertEqual(self.t.snapshot_from_dir(d, "gone").rows, [])

    def test_the_seam_never_shells_out(self):
        import inspect
        for fn in (self.t.resolve_source, self.t.snapshot_from_dir,
                   self.t.snapshot_from_text):
            self.assertNotIn("subprocess", inspect.getsource(fn))


# --------------------------------------------------------------------------- #
# the `diff` subcommand (#q9cq65c)
# --------------------------------------------------------------------------- #
class TestDiffCommand(DiffTestCase):
    def run_diff(self, d, **over):
        args = dict(dir=str(d), **{"from": None, "to": None})
        args.update(over)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_diff(ns(**args))
        return buf.getvalue()

    def test_diffs_a_file_against_the_working_tree_without_git(self):
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, row("aaa", status="done"), row("bbb"))
            old = Path(tmp) / "old.jsonl"
            old.write_text(index_text(row("aaa")))
            out = self.run_diff(d, **{"from": str(old)})
            self.assertIn("aaa", out)   # status backlog -> done
            self.assertIn("bbb", out)   # added
            self.assertIn("done", out)

    def test_to_defaults_to_the_working_tree(self):
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, row("aaa", priority="high"))
            old = Path(tmp) / "old.jsonl"
            old.write_text(index_text(row("aaa")))
            self.assertIn("high", self.run_diff(d, **{"from": str(old)}))

    def test_both_sides_may_be_explicit(self):
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, row("zzz"))
            a, b = Path(tmp) / "a.jsonl", Path(tmp) / "b.jsonl"
            a.write_text(index_text(row("aaa")))
            b.write_text(index_text(row("aaa", priority="urgent")))
            out = self.run_diff(d, **{"from": str(a), "to": str(b)})
            self.assertIn("urgent", out)
            self.assertNotIn("zzz", out)  # the working tree was not consulted

    def test_identical_sides_report_no_changes(self):
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, row("aaa"))
            old = Path(tmp) / "old.jsonl"
            old.write_text(index_text(row("aaa")))
            self.assertIn("no changes", self.run_diff(d, **{"from": str(old)}).lower())


if __name__ == "__main__":
    unittest.main()
