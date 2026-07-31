"""`trck diff`'s git convenience layer (#wtmfdhr).

The seam itself (#q9cq65c) is tested from fixture files with no repository at
all — that is the point of it. This file is the counterpart: the one place a
real git repo, real commits, and `git show` are involved, because resolving a
revision into a snapshot is exactly what cannot be faked.
"""
import json
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import TRCK_PATH


def git(root, *args, check=True):
    r = subprocess.run(["git", *args], cwd=root, capture_output=True, text=True)
    if check and r.returncode != 0:
        raise AssertionError(f"git {' '.join(args)} failed:\n{r.stdout}\n{r.stderr}")
    return r


def trck(cwd, *args, tracker=None, env=None, check=True):
    cmd = [sys.executable, str(TRCK_PATH)]
    if tracker is not None:
        cmd += ["--dir", str(tracker)]
    r = subprocess.run(cmd + list(args), cwd=cwd, capture_output=True, text=True, env=env)
    if check and r.returncode != 0:
        raise AssertionError(f"trck {' '.join(args)} failed:\n{r.stdout}\n{r.stderr}")
    return r


def row(iid, **over):
    d = {"id": iid, "slug": f"i{iid}", "title": f"I{iid}", "kind": "task",
         "status": "backlog", "priority": "medium"}
    d.update(over)
    return d


class GitFixture(unittest.TestCase):
    """A repo whose tracker lives in issues/, unless `tracker_at_root`."""
    tracker_at_root = False

    def build(self, tmp, *rows, bodies=None):
        root = Path(tmp) / "repo"
        root.mkdir(parents=True)
        git(root.parent, "init", "-q", "-b", "main", str(root))
        git(root, "config", "user.email", "t@t")
        git(root, "config", "user.name", "t")
        self.tracker = root if self.tracker_at_root else root / "issues"
        (self.tracker / "items").mkdir(parents=True, exist_ok=True)
        (self.tracker / "trck.json").write_text("{}")
        self.write(*rows, bodies=bodies)
        git(root, "add", "-A")
        git(root, "commit", "-qm", "seed")
        return root

    def write(self, *rows, bodies=None):
        (self.tracker / "index.jsonl").write_text(
            "".join(json.dumps(r) + "\n" for r in rows))
        for iid, text in (bodies or {}).items():
            (self.tracker / "items" / f"{iid}-i{iid}.md").write_text(text)

    def commit(self, root, msg="change"):
        git(root, "add", "-A")
        git(root, "commit", "-qm", msg)


class TestRevisionSources(GitFixture):
    def test_no_arguments_diffs_head_against_the_working_tree(self):
        with TemporaryDirectory() as tmp:
            root = self.build(tmp, row("aaa"))
            self.write(row("aaa", status="done"))  # uncommitted
            out = trck(root, "diff").stdout
            self.assertIn("HEAD", out)
            self.assertIn("working tree", out)
            self.assertIn("done", out)

    def test_no_changes_when_the_working_tree_matches_head(self):
        with TemporaryDirectory() as tmp:
            root = self.build(tmp, row("aaa"))
            self.assertIn("no changes", trck(root, "diff").stdout.lower())

    def test_a_revision_spec_diffs_that_revision_against_the_working_tree(self):
        with TemporaryDirectory() as tmp:
            root = self.build(tmp, row("aaa"))
            self.write(row("aaa", priority="high"))
            self.commit(root)
            self.write(row("aaa", priority="urgent"))  # uncommitted
            out = trck(root, "diff", "HEAD~1").stdout
            self.assertIn("medium → urgent", out)

    def test_two_dot_form_sets_both_sides(self):
        with TemporaryDirectory() as tmp:
            root = self.build(tmp, row("aaa"))
            self.write(row("aaa", priority="high"))
            self.commit(root)
            self.write(row("aaa", priority="urgent"))  # uncommitted, must be ignored
            out = trck(root, "diff", "HEAD~1..HEAD").stdout
            self.assertIn("medium → high", out)
            self.assertNotIn("urgent", out)

    def test_branch_names_work_as_revisions(self):
        with TemporaryDirectory() as tmp:
            root = self.build(tmp, row("aaa"))
            git(root, "checkout", "-q", "-b", "feature")
            self.write(row("aaa"), row("bbb"))
            self.commit(root)
            out = trck(root, "diff", "main..feature").stdout
            self.assertIn("bbb", out)

    def test_an_added_issue_between_two_revisions(self):
        with TemporaryDirectory() as tmp:
            root = self.build(tmp, row("aaa"))
            self.write(row("aaa"), row("bbb"))
            self.commit(root)
            self.assertIn("+ #bbb", trck(root, "diff", "HEAD~1..HEAD").stdout)


class TestTrackerPathResolution(GitFixture):
    def test_runs_from_a_subdirectory_of_the_repo(self):
        with TemporaryDirectory() as tmp:
            root = self.build(tmp, row("aaa"))
            self.write(row("aaa", status="done"))
            sub = root / "src" / "deep"
            sub.mkdir(parents=True)
            self.assertIn("done", trck(sub, "diff").stdout)

    def test_tracker_dir_that_is_itself_the_repo_root(self):
        self.tracker_at_root = True
        with TemporaryDirectory() as tmp:
            root = self.build(tmp, row("aaa"))
            self.write(row("aaa", status="done"))
            self.assertIn("done", trck(root, "diff").stdout)


class TestAbsentAndBadRevisions(GitFixture):
    def test_tracker_absent_at_that_revision_reads_as_all_added(self):
        with TemporaryDirectory() as tmp:
            root = Path(tmp) / "repo"
            root.mkdir(parents=True)
            git(root.parent, "init", "-q", "-b", "main", str(root))
            git(root, "config", "user.email", "t@t")
            git(root, "config", "user.name", "t")
            (root / "README.md").write_text("no tracker yet\n")
            git(root, "add", "-A")
            git(root, "commit", "-qm", "before the tracker existed")
            self.tracker = root / "issues"
            (self.tracker / "items").mkdir(parents=True)
            (self.tracker / "trck.json").write_text("{}")
            self.write(row("aaa"))
            out = trck(root, "diff", "HEAD").stdout
            self.assertIn("+ #aaa", out)

    def test_unresolvable_revision_is_a_clean_error(self):
        with TemporaryDirectory() as tmp:
            root = self.build(tmp, row("aaa"))
            r = trck(root, "diff", "no-such-branch", check=False)
            self.assertEqual(r.returncode, 1)
            self.assertIn("no-such-branch", r.stderr)
            self.assertNotIn("Traceback", r.stderr)

    def test_outside_a_git_repository_points_at_the_from_flag(self):
        with TemporaryDirectory() as tmp:
            d = Path(tmp) / "issues"
            (d / "items").mkdir(parents=True)
            (d / "trck.json").write_text("{}")
            (d / "index.jsonl").write_text(json.dumps(row("aaa")) + "\n")
            r = trck(tmp, "diff", tracker=d, check=False)
            self.assertEqual(r.returncode, 1)
            self.assertIn("--from", r.stderr)
            self.assertNotIn("Traceback", r.stderr)

    def test_git_missing_from_path_points_at_the_from_flag(self):
        with TemporaryDirectory() as tmp:
            root = self.build(tmp, row("aaa"))
            r = trck(root, "diff", env={"PATH": "/nonexistent"}, check=False)
            self.assertEqual(r.returncode, 1)
            self.assertIn("git", r.stderr)
            self.assertIn("--from", r.stderr)
            self.assertNotIn("Traceback", r.stderr)

    def test_a_revision_and_an_explicit_source_conflict(self):
        with TemporaryDirectory() as tmp:
            root = self.build(tmp, row("aaa"))
            other = Path(tmp) / "other.jsonl"
            other.write_text(json.dumps(row("aaa")) + "\n")
            r = trck(root, "diff", "HEAD", "--from", str(other), check=False)
            self.assertEqual(r.returncode, 2)
            self.assertNotIn("Traceback", r.stderr)


class TestGitSnapshotBodies(GitFixture):
    def test_bodies_are_readable_from_a_revision(self):
        with TemporaryDirectory() as tmp:
            root = self.build(tmp, row("aaa"), bodies={"aaa": "# Iaaa\n\nold prose\n"})
            sys.path.insert(0, str(Path(TRCK_PATH).parent))
            try:
                from tests.helpers import load_trck
                t = load_trck()
                ctx = t.Ctx(self.tracker, t.load_config(self.tracker))
                snap = t.git_snapshot(ctx, "HEAD")
            finally:
                sys.path.pop(0)
            self.assertTrue(snap.has_bodies)
            self.assertEqual(snap.body("aaa"), "# Iaaa\n\nold prose\n")
            self.assertEqual([r.id for r in snap.rows], ["aaa"])

    def test_a_body_absent_at_that_revision_is_none(self):
        with TemporaryDirectory() as tmp:
            root = self.build(tmp, row("aaa"))  # no body file written
            from tests.helpers import load_trck
            t = load_trck()
            ctx = t.Ctx(self.tracker, t.load_config(self.tracker))
            self.assertIsNone(t.git_snapshot(ctx, "HEAD").body("aaa"))


if __name__ == "__main__":
    unittest.main()
