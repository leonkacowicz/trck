"""End-to-end: real git merges and rebases through the registered drivers (#2ry5d58).

Everything else in this feature is a pure function tested in isolation. This is the
only place the whole chain runs: `.gitattributes` naming a driver, `.git/config`
defining it, git invoking it mid-operation, and the result landing in a working tree.

**Every scenario runs in all three integration directions.** Two is not enough:
merging feature into main and rebasing feature onto main hand git the same operands,
so a driver that wrongly assumes `%A` is the user's side passes both and still breaks
on `git merge main` from the feature branch.
"""
import json
import subprocess
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import TRCK_PATH

DIRECTIONS = ("merge-into-main", "merge-into-feature", "rebase-onto-main")


def git(root, *args, check=True):
    r = subprocess.run(["git", *args], cwd=root, capture_output=True, text=True)
    if check and r.returncode != 0:
        raise AssertionError(f"git {' '.join(args)} failed:\n{r.stdout}\n{r.stderr}")
    return r


def trck(root, *args, check=True):
    r = subprocess.run(["python3", str(TRCK_PATH), "--dir", str(root / "issues"), *args],
                       cwd=root, capture_output=True, text=True)
    if check and r.returncode != 0:
        raise AssertionError(f"trck {' '.join(args)} failed:\n{r.stdout}\n{r.stderr}")
    return r


class MergeScenario(unittest.TestCase):
    """Builds a repo, applies a divergence, then integrates it three ways."""

    def build(self, tmp, seed_rows):
        root = Path(tmp) / "repo"
        (root / "issues" / "items").mkdir(parents=True)
        (root / "issues" / "trck.json").write_text("{}")
        git(root.parent, "init", "-q", "-b", "main", str(root))
        git(root, "config", "user.email", "t@t")
        git(root, "config", "user.name", "t")
        self.write_index(root, seed_rows)
        for r in seed_rows:
            (root / "issues" / "items" / f"{r['id']}-{r['slug']}.md").write_text(
                f"# {r['title']}\n")
        trck(root, "repo", "setup-git")
        trck(root, "summary")
        git(root, "add", "-A")
        git(root, "commit", "-qm", "seed")
        return root

    def write_index(self, root, rows):
        (root / "issues" / "index.jsonl").write_text(
            "".join(json.dumps(r) + "\n" for r in sorted(rows, key=lambda r: r["id"])))

    def rows_of(self, root):
        text = (root / "issues" / "index.jsonl").read_text()
        return [json.loads(l) for l in text.splitlines() if l.strip()]

    def diverge(self, root, main_rows, feature_rows):
        """Commit `feature_rows` on a feature branch and `main_rows` on main."""
        git(root, "checkout", "-qb", "feature")
        self.write_index(root, feature_rows)
        git(root, "commit", "-qam", "feature side")
        git(root, "checkout", "-q", "main")
        self.write_index(root, main_rows)
        git(root, "commit", "-qam", "main side")

    def integrate(self, root, direction):
        """Run one of the three integrations. Returns the git result."""
        if direction == "merge-into-main":
            git(root, "checkout", "-q", "main")
            return git(root, "merge", "--no-edit", "feature", check=False)
        if direction == "merge-into-feature":
            git(root, "checkout", "-q", "feature")
            return git(root, "merge", "--no-edit", "main", check=False)
        git(root, "checkout", "-q", "feature")
        return git(root, "rebase", "main", check=False)

    def scenario(self, tmp, direction, seed, main_rows, feature_rows):
        root = self.build(tmp, seed)
        self.diverge(root, main_rows, feature_rows)
        result = self.integrate(root, direction)
        return root, result


def row(iid, **over):
    r = {"id": iid, "slug": "alpha", "title": "Alpha", "kind": "task",
         "status": "backlog", "priority": "medium"}
    r.update(over)
    return r


class TestCleanMergeAllDirections(MergeScenario):
    """Each side creates a different issue — must resolve with no human input."""

    def test_disjoint_creates_resolve_cleanly(self):
        for direction in DIRECTIONS:
            with self.subTest(direction=direction), TemporaryDirectory() as tmp:
                seed = [row("seed0000", slug="seed", title="Seed")]
                root, result = self.scenario(
                    tmp, direction, seed,
                    main_rows=seed + [row("mmm1111", slug="from-main", title="From Main")],
                    feature_rows=seed + [row("fff2222", slug="from-feat", title="From Feat")])
                self.assertEqual(result.returncode, 0,
                                 f"{direction} should not conflict:\n{result.stdout}{result.stderr}")
                ids = sorted(r["id"] for r in self.rows_of(root))
                self.assertEqual(ids, ["fff2222", "mmm1111", "seed0000"])

    def test_summary_is_regenerated_after_a_clean_merge(self):
        """Whichever driver git runs first, the rollup reflects the merged rows."""
        for direction in DIRECTIONS:
            with self.subTest(direction=direction), TemporaryDirectory() as tmp:
                seed = [row("seed0000", slug="seed", title="Seed")]
                root, result = self.scenario(
                    tmp, direction, seed,
                    main_rows=seed + [row("mmm1111", slug="from-main", title="From Main")],
                    feature_rows=seed + [row("fff2222", slug="from-feat", title="From Feat")])
                self.assertEqual(result.returncode, 0)
                summary = (root / "issues" / "SUMMARY.md").read_text()
                self.assertIn("From Main", summary)
                self.assertIn("From Feat", summary)

    def test_independent_field_edits_on_one_issue_resolve_cleanly(self):
        for direction in DIRECTIONS:
            with self.subTest(direction=direction), TemporaryDirectory() as tmp:
                seed = [row("aaa1111", title="Original", priority="medium")]
                root, result = self.scenario(
                    tmp, direction, seed,
                    main_rows=[row("aaa1111", title="Original", priority="urgent")],
                    feature_rows=[row("aaa1111", title="Renamed", priority="medium")])
                self.assertEqual(result.returncode, 0,
                                 f"{direction}:\n{result.stdout}{result.stderr}")
                merged = self.rows_of(root)[0]
                self.assertEqual(merged["priority"], "urgent")
                self.assertEqual(merged["title"], "Renamed")

    def test_labels_union_across_a_real_merge(self):
        for direction in DIRECTIONS:
            with self.subTest(direction=direction), TemporaryDirectory() as tmp:
                seed = [row("aaa1111", labels=["keep"])]
                root, result = self.scenario(
                    tmp, direction, seed,
                    main_rows=[row("aaa1111", labels=["keep", "from-main"])],
                    feature_rows=[row("aaa1111", labels=["keep", "from-feat"])])
                self.assertEqual(result.returncode, 0)
                self.assertEqual(sorted(self.rows_of(root)[0]["labels"]),
                                 ["from-feat", "from-main", "keep"])


class TestConflictAllDirections(MergeScenario):
    """Both sides move the same leaf — must stop, in every direction."""

    def test_lifecycle_divergence_conflicts(self):
        for direction in DIRECTIONS:
            with self.subTest(direction=direction), TemporaryDirectory() as tmp:
                seed = [row("aaa1111", status="backlog")]
                _, result = self.scenario(
                    tmp, direction, seed,
                    main_rows=[row("aaa1111", status="ongoing")],
                    feature_rows=[row("aaa1111", status="done", closed="2026-01-01T00:00:00Z")])
                self.assertNotEqual(result.returncode, 0,
                                    f"{direction} must not auto-resolve a real disagreement")

    def test_conflicted_index_cannot_be_committed_unread(self):
        """The file carries markers, so it does not parse — `trck check` refuses."""
        for direction in DIRECTIONS:
            with self.subTest(direction=direction), TemporaryDirectory() as tmp:
                seed = [row("aaa1111", status="backlog")]
                root, _ = self.scenario(
                    tmp, direction, seed,
                    main_rows=[row("aaa1111", status="ongoing")],
                    feature_rows=[row("aaa1111", status="done", closed="2026-01-01T00:00:00Z")])
                text = (root / "issues" / "index.jsonl").read_text()
                self.assertIn("<<<<<<<", text)
                self.assertNotEqual(trck(root, "check", check=False).returncode, 0)

    def test_the_documented_tuple_corruption_never_materialises(self):
        """One side sets a resolution without moving status, the other reopens.
        Field-wise this looks clean and yields status=ongoing + resolution=wontfix,
        a row no verb can write. It must conflict in every direction instead."""
        for direction in DIRECTIONS:
            with self.subTest(direction=direction), TemporaryDirectory() as tmp:
                seed = [row("aaa1111", status="done", closed="T1")]
                root, result = self.scenario(
                    tmp, direction, seed,
                    main_rows=[row("aaa1111", status="done", closed="T1",
                                   resolution="wontfix")],
                    feature_rows=[row("aaa1111", status="backlog")])
                self.assertNotEqual(result.returncode, 0, direction)
                text = (root / "issues" / "index.jsonl").read_text()
                for line in text.splitlines():
                    if line.strip().startswith("{"):
                        r = json.loads(line)
                        self.assertFalse(
                            r.get("status") == "backlog" and r.get("resolution"),
                            f"{direction} produced the forbidden row: {line}")


class TestUnregisteredCloneFallsBack(MergeScenario):
    """`.gitattributes` is shared but driver commands are not, so a clone that never
    ran `setup-git` is the normal case, not an edge case. Both un-registered states
    must fail safely — the requirement is that neither silently picks a side."""

    def diverged_repo(self, tmp):
        root = self.build(tmp, [row("aaa1111", status="backlog")])
        self.diverge(root,
                     main_rows=[row("aaa1111", status="ongoing")],
                     feature_rows=[row("aaa1111", status="done", closed="T1")])
        return root

    def test_fresh_clone_falls_back_to_an_ordinary_three_way_merge(self):
        """Nothing registered at all: git uses its built-in merge and writes normal
        conflict markers. An un-set-up clone is exactly as well off as before."""
        with TemporaryDirectory() as tmp:
            root = self.build(tmp, [row("aaa1111", status="backlog")])
            git(root, "config", "--remove-section", "merge.trck-index")
            self.diverge(root,
                         main_rows=[row("aaa1111", status="ongoing")],
                         feature_rows=[row("aaa1111", status="done", closed="T1")])
            result = self.integrate(root, "merge-into-main")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("<<<<<<<", (root / "issues" / "index.jsonl").read_text())

    def test_half_registered_driver_aborts_loudly_and_changes_nothing(self):
        """A `merge.<name>.name` with no `.driver` — a partially edited config, or a
        future rename. Git refuses outright rather than guessing, and leaves the
        working tree untouched. Different from a fresh clone, equally safe."""
        with TemporaryDirectory() as tmp:
            root = self.diverged_repo(tmp)
            git(root, "config", "--unset", "merge.trck-index.driver")
            before = (root / "issues" / "index.jsonl").read_text()
            result = self.integrate(root, "merge-into-main")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("lacks command line", result.stderr)
            self.assertEqual((root / "issues" / "index.jsonl").read_text(), before)
