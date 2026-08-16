"""Tests for scripts/ci_changed.py — the CI path classifier.

The script decides whether a change can skip the engine's suites. Getting that wrong
in the *skip* direction means a pull request merges without ever being built, and the
failure is silent: the checks go green because they never ran. So the interesting
assertions here are the ones that insist a path counts as code.

    python3 -m unittest discover -s scripts/tests
"""
import subprocess
import sys
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT_PATH = REPO_ROOT / "scripts" / "ci_changed.py"


def load_ci_changed():
    """Import the script as a fresh module object."""
    import importlib.machinery
    import importlib.util
    loader = importlib.machinery.SourceFileLoader("ci_changed", str(SCRIPT_PATH))
    spec = importlib.util.spec_from_file_location("ci_changed", SCRIPT_PATH, loader=loader)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["ci_changed"] = mod
    spec.loader.exec_module(mod)
    return mod


class SkippablePaths(unittest.TestCase):
    """What a change may touch and still skip the engine's suites."""

    def setUp(self):
        self.mod = load_ci_changed()

    def test_docs_are_skippable(self):
        self.assertFalse(self.mod.needs_full_ci(["docs/specs/dates.md", "docs/img/ready.svg"]))

    def test_root_markdown_is_skippable(self):
        self.assertFalse(self.mod.needs_full_ci(["README.md", "CONTRIBUTING.md", "CLAUDE.md"]))


class CodePaths(unittest.TestCase):
    """What must never skip. Each of these is markdown or prose that the engine or its
    specification reads, which is why the rule is an allowlist of directories rather than
    a denylist keyed on the extension."""

    def setUp(self):
        self.mod = load_ci_changed()

    def test_engine_sources_are_code(self):
        self.assertTrue(self.mod.needs_full_ci(["src/cli.rs"]))

    def test_the_tracker_directory_is_code(self):
        """`issues/` used to be the first entry on the allowlist. It is not one any more.

        The tracker lives on the `trck-issues` branch, where a pull request against `main`
        structurally cannot reach it — so a diff that touches `issues/` no longer means "a
        tracker edit" and cannot be assumed inert. What it means now is that somebody put a
        directory of that name back in the tree, which is precisely the change that should be
        built rather than waved through."""
        self.assertTrue(self.mod.needs_full_ci([
            "issues/index.jsonl",
            "issues/SUMMARY.md",
            "issues/items/nbjnx54-a-thing.md",
        ]))
        self.assertTrue(self.mod.needs_full_ci(["issues/CLAUDE.md"]))

    def test_a_conformance_fixture_is_code(self):
        self.assertTrue(self.mod.needs_full_ci([
            "conformance/fixtures/list-orders-by-priority/expected.out",
        ]))

    def test_a_conformance_fixtures_markdown_is_code(self):
        self.assertTrue(self.mod.needs_full_ci([
            "conformance/fixtures/changelog-ignores-open-issues/initial/items/aaaaaaa-done1.md",
        ]))

    def test_a_compiled_in_asset_is_code(self):
        # assets/ is compiled into the binary — scaffold-CLAUDE.md included.
        self.assertTrue(self.mod.needs_full_ci(["assets/scaffold-CLAUDE.md"]))
        self.assertTrue(self.mod.needs_full_ci(["assets/app.js"]))

    def test_an_example_tracker_is_code(self):
        self.assertTrue(self.mod.needs_full_ci(["examples/action-game/SUMMARY.md"]))

    def test_the_skill_is_code(self):
        self.assertTrue(self.mod.needs_full_ci(["skills/trck/SKILL.md"]))

    def test_the_helper_scripts_are_code(self):
        self.assertTrue(self.mod.needs_full_ci(["scripts/install.sh"]))
        self.assertTrue(self.mod.needs_full_ci(["scripts/ci_changed.py"]))

    def test_the_workflows_are_code(self):
        # A workflow edit must run under the workflow it edits, or the change that
        # widens the skip rule is the one change the skip rule is never checked on.
        self.assertTrue(self.mod.needs_full_ci([".github/workflows/ci.yml"]))

    def test_the_manifest_is_code(self):
        self.assertTrue(self.mod.needs_full_ci(["Cargo.toml"]))
        self.assertTrue(self.mod.needs_full_ci(["quality-report.json"]))

    def test_one_code_path_among_many_skippable_ones_is_code(self):
        self.assertTrue(self.mod.needs_full_ci([
            "docs/specs/dates.md",
            "README.md",
            "src/config.rs",
        ]))

    def test_a_directory_that_merely_starts_with_a_skippable_name_is_code(self):
        self.assertTrue(self.mod.needs_full_ci(["docsite/index.html"]))
        self.assertTrue(self.mod.needs_full_ci(["docs-archive/thing.md"]))


class FailSafe(unittest.TestCase):
    """Every uncertainty resolves to *run everything*. A classifier that skips when it
    does not know is a classifier that eventually skips everything."""

    def setUp(self):
        self.mod = load_ci_changed()

    def test_an_empty_diff_is_code(self):
        self.assertTrue(self.mod.needs_full_ci([]))

    def test_blank_lines_alone_are_code(self):
        self.assertTrue(self.mod.needs_full_ci(["", "  ", "\t"]))

    def test_blank_lines_do_not_hide_a_skippable_verdict(self):
        self.assertFalse(self.mod.needs_full_ci(["", "docs/specs/dates.md", ""]))

    def test_a_root_file_that_is_not_markdown_is_code(self):
        self.assertTrue(self.mod.needs_full_ci(["LICENSE"]))
        self.assertTrue(self.mod.needs_full_ci([".gitignore"]))


class CommandLine(unittest.TestCase):
    """The workflow reads one word on stdout and nothing else."""

    def run_script(self, stdin):
        return subprocess.run(
            [sys.executable, str(SCRIPT_PATH)],
            input=stdin, capture_output=True, text=True, cwd=REPO_ROOT,
        )

    def test_prints_false_for_a_skippable_diff(self):
        proc = self.run_script("docs/specs/dates.md\nREADME.md\n")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stdout, "false\n")

    def test_prints_true_for_a_code_diff(self):
        proc = self.run_script("src/cli.rs\n")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stdout, "true\n")

    def test_prints_true_for_no_input(self):
        proc = self.run_script("")
        self.assertEqual(proc.returncode, 0, proc.stderr)
        self.assertEqual(proc.stdout, "true\n")


class WorkflowWiring(unittest.TestCase):
    """The classifier is only worth anything if the workflow actually consults it, and only
    safe if every required check still reports when the answer is "skip"."""

    def setUp(self):
        self.ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")
        # Each job's own block, so a claim about one job cannot be satisfied by another.
        self.jobs = {}
        name = None
        for line in self.ci.splitlines():
            if len(line) > 2 and line[2] != " " and line[2] != "#" and line.rstrip().endswith(":"):
                name = line.strip().rstrip(":")
                self.jobs[name] = []
            elif name:
                self.jobs[name].append(line)
        self.jobs = {k: "\n".join(v) for k, v in self.jobs.items()}

    # assertIn would print the whole workflow on failure; the claim is short, so say it.
    def contains(self, haystack, needle, claim):
        self.assertTrue(needle in haystack, f"ci.yml: {claim} — no `{needle}`")

    def lacks(self, haystack, needle, claim):
        self.assertTrue(needle not in haystack, f"ci.yml: {claim} — found `{needle}`")

    def test_the_workflow_calls_the_classifier(self):
        self.contains(self.ci, "scripts/ci_changed.py", "the changes job consults the classifier")

    def test_every_job_level_gate_reads_the_same_output(self):
        gate = "if: needs.changes.outputs.code == 'true'"
        # Exact lines: a job-level gate is indented four, a step-level one eight.
        job_level = [ln for ln in self.ci.splitlines() if ln == f"    {gate}"]
        self.assertEqual(len(job_level), 3,
                         f"ci.yml: scripts, quality and installer each gated on `{gate}`")
        for job in ("scripts", "quality", "installer"):
            self.contains(self.jobs[job], gate, f"the {job} job is gated")

    def test_the_matrix_job_is_never_skipped_as_a_whole(self):
        """A matrix job skipped by an `if:` reports once, under the bare job name — the
        per-combination check `rust (ubuntu-latest)` that merging is gated on never arrives,
        and the pull request waits for it indefinitely. So `rust` shrinks its matrix and
        skips steps instead."""
        rust = self.jobs["rust"]
        self.assertNotIn("\n    if:", rust, "ci.yml: the rust job must carry no job-level if:")
        self.contains(rust, "os: ${{ fromJSON(needs.changes.outputs.matrix) }}",
                      "the rust job shrinks its matrix instead")
        self.contains(self.jobs["changes"], "matrix=$matrix",
                      "the changes job publishes that matrix")

    def test_the_code_workflow_does_not_know_the_tracker_exists(self):
        """The tracker is not in this workflow's checkout any more, so a step that looked for
        it would be checking whatever happened to be at that path — nothing, on a good day."""
        self.lacks(self.ci, "issues/", "no path filter or step may name the tracker directory")
        self.lacks(self.ci, "trck check", "the tracker check lives in its own workflow")

    def test_the_matrix_job_still_builds_whatever_the_classifier_said(self):
        """It cannot be skipped as a whole (see above), so on a prose-only change it runs and
        does the one thing left that is worth doing: prove the engine still builds."""
        body = self.jobs["rust"].split("- name: Build", 1)[1].split("- name:", 1)[0]
        self.assertNotIn("needs.changes.outputs.code", body,
                         "ci.yml: `- name: Build` must run whatever the classifier said")


class TrackerWorkflow(unittest.TestCase):
    """The tracker gets its own workflow, on its own branch.

    It cannot be a job in `ci.yml`: that workflow runs on pull requests to `main` and pushes
    to it, and the tracker branch shares no history with either. And it must not become a
    required check on `main` — a branch protection rule waiting for a job that only ever
    fires on `trck-issues` is a pull request that never merges."""

    def setUp(self):
        path = REPO_ROOT / ".github" / "workflows" / "tracker.yml"
        self.assertTrue(path.is_file(), "no .github/workflows/tracker.yml")
        self.yml = path.read_text(encoding="utf-8")

    def test_it_fires_on_a_push_to_the_tracker_branch(self):
        self.assertIn("branches: [trck-issues]", self.yml)

    def test_it_runs_check_against_the_pushed_commit(self):
        self.assertIn("trck --ref", self.yml, "the check must read the branch, not a checkout")
        self.assertIn("check", self.yml)

    def test_it_builds_the_engine_from_this_repository(self):
        """Not a downloaded release: the tracker is answered for by the engine in `main`, for
        the same reason nothing here is vendored."""
        self.assertIn("cargo build --release", self.yml)


if __name__ == "__main__":
    unittest.main()
