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

    def test_tracker_data_is_skippable(self):
        self.assertFalse(self.mod.needs_full_ci([
            "issues/index.jsonl",
            "issues/SUMMARY.md",
            "issues/items/nbjnx54-a-thing.md",
        ]))

    def test_the_trackers_own_instructions_are_skippable(self):
        # Under issues/, so it rides along with the tracker rather than needing its own rule.
        self.assertFalse(self.mod.needs_full_ci(["issues/CLAUDE.md"]))

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
            "issues/index.jsonl",
            "README.md",
            "src/config.rs",
        ]))

    def test_a_directory_that_merely_starts_with_a_skippable_name_is_code(self):
        self.assertTrue(self.mod.needs_full_ci(["issues-archive/thing.md"]))
        self.assertTrue(self.mod.needs_full_ci(["docsite/index.html"]))


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
        self.assertFalse(self.mod.needs_full_ci(["", "issues/index.jsonl", ""]))

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
        proc = self.run_script("issues/index.jsonl\nissues/SUMMARY.md\n")
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
    """The classifier is only worth anything if the workflow actually consults it, and
    the required jobs are the ones that have to be gated on the answer."""

    def setUp(self):
        self.ci = (REPO_ROOT / ".github" / "workflows" / "ci.yml").read_text(encoding="utf-8")

    # assertIn would print the whole workflow on failure; the claim is short, so say it.
    def contains(self, needle, claim):
        self.assertTrue(needle in self.ci, f"ci.yml: {claim} — no `{needle}`")

    def test_the_workflow_calls_the_classifier(self):
        self.contains("scripts/ci_changed.py", "the changes job consults the classifier")

    def test_every_gated_job_is_gated_on_the_same_output(self):
        gate = "if: needs.changes.outputs.code == 'true'"
        self.assertEqual(self.ci.count(gate), 4,
                         f"ci.yml: scripts, quality, installer and rust each gated on `{gate}`")

    def test_the_tracker_is_checked_when_the_engine_jobs_are_skipped(self):
        self.contains("if: needs.changes.outputs.code == 'false'",
                      "a job takes over trck check when rust is skipped")
        self.contains("trck check", "the tracker is checked somewhere")


if __name__ == "__main__":
    unittest.main()
