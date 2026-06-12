import json
import os
import shutil
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_backfill


class TestToUtc(unittest.TestCase):
    def setUp(self):
        self.b = load_backfill()

    def test_to_utc_converts_offset_to_utc(self):
        self.assertEqual(self.b.to_utc("2026-06-06T12:34:56-03:00"), "2026-06-06T15:34:56Z")

    def test_to_utc_passes_through_utc(self):
        self.assertEqual(self.b.to_utc("2026-06-06T00:00:00+00:00"), "2026-06-06T00:00:00Z")

    def test_to_utc_normalizes_across_day_boundary(self):
        self.assertEqual(self.b.to_utc("2026-06-06T23:30:00-03:00"), "2026-06-07T02:30:00Z")


class TestRewriteLines(unittest.TestCase):
    def setUp(self):
        self.b = load_backfill()

    def canonical(self, **row):
        return json.dumps(row, ensure_ascii=False)

    def test_is_day_only(self):
        self.assertTrue(self.b.is_day_only("2026-06-05"))
        self.assertFalse(self.b.is_day_only("2026-06-05T00:00:00Z"))
        self.assertFalse(self.b.is_day_only(None))
        self.assertFalse(self.b.is_day_only(""))

    def test_rewrite_replaces_day_only_and_leaves_others_byte_identical(self):
        recovered = {
            (1, "created"): "2026-06-05T09:00:00+00:00",
            (1, "closed"): "2026-06-06T12:00:00-03:00",
        }
        line1 = self.canonical(id=1, slug="x", title="X", kind="task",
                               status="done", priority="medium",
                               created="2026-06-05", closed="2026-06-06")
        line2 = self.canonical(id=2, slug="y", title="Y", kind="task",
                               status="backlog", priority="low",
                               created="2026-06-05T08:00:00Z")
        new_lines, changes, warnings = self.b.rewrite_lines([line1, line2], recovered)
        # issue 2 already timestamped -> untouched and byte-identical
        self.assertEqual(new_lines[1], line2)
        # issue 1's day-only fields converted to UTC
        row1 = json.loads(new_lines[0])
        self.assertEqual(row1["created"], "2026-06-05T09:00:00Z")
        self.assertEqual(row1["closed"], "2026-06-06T15:00:00Z")
        self.assertEqual(set(changes), {
            (1, "created", "2026-06-05", "2026-06-05T09:00:00Z"),
            (1, "closed", "2026-06-06", "2026-06-06T15:00:00Z"),
        })
        self.assertEqual(warnings, [])

    def test_rewrite_warns_when_no_history_and_leaves_value(self):
        line = self.canonical(id=3, slug="z", title="Z", kind="task",
                              status="done", priority="medium", created="2026-06-05")
        new_lines, changes, warnings = self.b.rewrite_lines([line], {})
        self.assertEqual(new_lines[0], line)  # unchanged
        self.assertEqual(changes, [])
        self.assertEqual(warnings, [(3, "created", "2026-06-05")])

    def test_rewrite_preserves_blank_lines(self):
        new_lines, changes, warnings = self.b.rewrite_lines(["", "  "], {})
        self.assertEqual(new_lines, ["", "  "])
        self.assertEqual(changes, [])
        self.assertEqual(warnings, [])


class TestReduceTransitions(unittest.TestCase):
    def setUp(self):
        self.b = load_backfill()

    def test_first_appearance_sets_created(self):
        snaps = [("2026-06-01T10:00:00+00:00", [{"id": 1, "created": "2026-06-01"}])]
        rec = self.b.reduce_transitions(snaps)
        self.assertEqual(rec[(1, "created")], "2026-06-01T10:00:00+00:00")

    def test_last_close_wins_after_reopen(self):
        snaps = [
            ("2026-06-01T10:00:00+00:00", [{"id": 1, "created": "2026-06-01"}]),
            ("2026-06-02T10:00:00+00:00", [{"id": 1, "created": "2026-06-01", "closed": "2026-06-02"}]),
            ("2026-06-03T10:00:00+00:00", [{"id": 1, "created": "2026-06-01"}]),               # reopened: closed cleared
            ("2026-06-04T10:00:00+00:00", [{"id": 1, "created": "2026-06-01", "closed": "2026-06-04"}]),  # reclosed
        ]
        rec = self.b.reduce_transitions(snaps)
        self.assertEqual(rec[(1, "closed")], "2026-06-04T10:00:00+00:00")
        self.assertEqual(rec[(1, "created")], "2026-06-01T10:00:00+00:00")

    def test_clear_to_none_keeps_last_set_time(self):
        snaps = [
            ("2026-06-02T10:00:00+00:00", [{"id": 1, "closed": "2026-06-02"}]),
            ("2026-06-03T10:00:00+00:00", [{"id": 1}]),  # cleared
        ]
        rec = self.b.reduce_transitions(snaps)
        self.assertEqual(rec[(1, "closed")], "2026-06-02T10:00:00+00:00")

    def test_non_integer_id_rows_are_ignored(self):
        snaps = [("2026-06-01T10:00:00+00:00", [{"slug": "noid", "created": "2026-06-01"}])]
        rec = self.b.reduce_transitions(snaps)
        self.assertEqual(rec, {})


def _git(cwd, *args):
    env = dict(
        os.environ,
        GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@e",
        GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@e",
    )
    subprocess.run(["git", "-C", str(cwd), *args], check=True,
                   capture_output=True, text=True, env=env)


def _commit_index(repo, content, author_iso):
    """Write issues/index.jsonl and commit it with a fixed author/committer date."""
    issues = Path(repo) / "issues"
    issues.mkdir(exist_ok=True)
    (issues / "index.jsonl").write_text(content)
    env = dict(
        os.environ,
        GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@e",
        GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@e",
        GIT_AUTHOR_DATE=author_iso, GIT_COMMITTER_DATE=author_iso,
    )
    subprocess.run(["git", "-C", str(repo), "add", "issues/index.jsonl"],
                   check=True, capture_output=True, text=True, env=env)
    subprocess.run(["git", "-C", str(repo), "-c", "commit.gpgsign=false",
                    "commit", "-m", "x"],
                   check=True, capture_output=True, text=True, env=env)


def _row(iid, **extra):
    row = {"id": iid, "slug": f"i{iid}", "title": f"I{iid}", "kind": "task",
           "status": "backlog", "priority": "medium"}
    row.update(extra)
    return json.dumps(row, ensure_ascii=False)


@unittest.skipUnless(shutil.which("git"), "git not available")
class TestGitLayer(unittest.TestCase):
    def setUp(self):
        self.b = load_backfill()

    def test_resolve_index_missing_file(self):
        with TemporaryDirectory() as tmp:
            with self.assertRaises(SystemExit):
                self.b.resolve_index(tmp)

    def test_resolve_index_not_a_repo(self):
        with TemporaryDirectory() as tmp:
            (Path(tmp) / "index.jsonl").write_text("")
            with self.assertRaises(SystemExit):
                self.b.resolve_index(tmp)

    def test_recover_times_over_history(self):
        with TemporaryDirectory() as tmp:
            _git(tmp, "init", "-q")
            _commit_index(tmp, _row(1, created="2026-06-01") + "\n",
                          "2026-06-01T09:00:00+00:00")
            _commit_index(tmp, _row(1, created="2026-06-01", started="2026-06-02",
                                    status="ongoing") + "\n",
                          "2026-06-02T12:00:00-03:00")
            _commit_index(tmp, _row(1, created="2026-06-01", started="2026-06-02",
                                    closed="2026-06-03", status="done") + "\n",
                          "2026-06-03T10:00:00+00:00")
            root, index_rel, _ = self.b.resolve_index(str(Path(tmp) / "issues"))
            rec = self.b.recover_times(root, index_rel)
            self.assertEqual(rec[(1, "created")], "2026-06-01T09:00:00+00:00")
            self.assertEqual(rec[(1, "started")], "2026-06-02T12:00:00-03:00")
            self.assertEqual(rec[(1, "closed")], "2026-06-03T10:00:00+00:00")


if __name__ == "__main__":
    unittest.main()
