"""Tests for scripts/ratchet_baseline.sh — the measured-baseline ratchet comparison.

The script is ten lines of glue, and glue is exactly what fails silently: an argument in
the wrong order, an unquoted path, a baseline generated in one directory and read from
another. CI would still go green, because `ratchet compare` given a baseline it agrees
with reports success — the gate would simply have stopped comparing anything.

So these tests put a stub `ratchet` on PATH, run the script against a real throwaway git
repository, and assert on the commands it issued and the tree it measured.

    python3 -m unittest discover -s scripts/tests
"""
import json
import os
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT = REPO_ROOT / "scripts" / "ratchet_baseline.sh"

# Records its argv, and writes the report `generate` is expected to produce so the
# `compare` call that follows has something to read.
STUB = """#!/bin/sh
printf '%s\\n' "$*" >> "$RATCHET_LOG"
if [ "$1" = generate ]; then
    printf '{"stub": true}' > "$3/quality-report.json"
fi
exit 0
"""


def git(cwd, *args):
    subprocess.run(
        ["git", *args],
        cwd=cwd,
        check=True,
        capture_output=True,
        env={**os.environ, "GIT_CONFIG_GLOBAL": os.devnull, "GIT_CONFIG_SYSTEM": os.devnull},
    )


class RatchetBaseline(unittest.TestCase):
    """What the script measures, and what it hands to `compare`."""

    def setUp(self):
        if not SCRIPT.exists():
            self.skipTest("script missing")
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        self.addCleanup(self.tmp.cleanup)

        self.repo = self.root / "repo"
        self.repo.mkdir()
        git(self.repo, "init", "-q", "-b", "main")
        git(self.repo, "config", "user.email", "t@example.test")
        git(self.repo, "config", "user.name", "t")
        (self.repo / "kept.txt").write_text("from the base commit\n")
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-qm", "base")
        self.base_rev = subprocess.run(
            ["git", "rev-parse", "HEAD"], cwd=self.repo, capture_output=True, text=True, check=True
        ).stdout.strip()
        # A second commit, so "the base" is genuinely not the working tree.
        (self.repo / "later.txt").write_text("added after the base\n")
        git(self.repo, "add", "-A")
        git(self.repo, "commit", "-qm", "later")

        self.bin = self.root / "bin"
        self.bin.mkdir()
        stub = self.bin / "ratchet"
        stub.write_text(STUB)
        stub.chmod(0o755)
        self.log = self.root / "calls.log"
        self.scratch = self.root / "scratch"
        self.scratch.mkdir()

    def run_script(self, rev):
        env = {
            **os.environ,
            "PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}",
            "RATCHET_LOG": str(self.log),
            "TMPDIR": str(self.scratch),
        }
        return subprocess.run(
            ["sh", str(SCRIPT), rev], cwd=self.repo, env=env, capture_output=True, text=True
        )

    def calls(self):
        return self.log.read_text().splitlines()

    def test_it_generates_the_baseline_then_compares_against_that_file(self):
        result = self.run_script(self.base_rev)
        self.assertEqual(result.returncode, 0, result.stderr)
        gen, cmp = self.calls()
        base = f"{self.scratch}/ratchet-baseline"
        self.assertEqual(gen, f"generate --root {base}")
        # `--root .` and not the baseline: the thing under test is the working tree.
        self.assertEqual(cmp, f"compare --root . --base-file {base}/quality-report.json")

    def test_the_baseline_tree_is_the_named_revision_not_the_working_tree(self):
        # The whole point of measuring rather than reading a committed report: get it
        # wrong and the gate compares the branch against itself and always passes.
        self.run_script(self.base_rev)
        base = self.scratch / "ratchet-baseline"
        self.assertTrue((base / "kept.txt").is_file(), "base tree not extracted")
        self.assertFalse((base / "later.txt").exists(), "extracted the working tree, not the base revision")

    def test_a_stale_baseline_directory_is_replaced_not_merged(self):
        # A runner that reuses its temp directory would otherwise measure a mix of two
        # revisions, which reads as a regression nobody introduced.
        base = self.scratch / "ratchet-baseline"
        base.mkdir(parents=True)
        (base / "stale.txt").write_text("left over from an earlier run\n")
        self.run_script(self.base_rev)
        self.assertFalse((base / "stale.txt").exists(), "stale baseline survived")

    def test_an_unknown_revision_fails_rather_than_comparing_nothing(self):
        result = self.run_script("no-such-rev")
        self.assertNotEqual(result.returncode, 0, "an unresolvable revision passed silently")
        self.assertFalse(self.log.exists(), "ratchet ran despite having no baseline tree")

    def test_it_refuses_without_a_revision(self):
        env = {**os.environ, "PATH": f"{self.bin}{os.pathsep}{os.environ['PATH']}", "RATCHET_LOG": str(self.log)}
        result = subprocess.run(["sh", str(SCRIPT)], cwd=self.repo, env=env, capture_output=True, text=True)
        self.assertNotEqual(result.returncode, 0)
        self.assertIn("usage", result.stderr.lower())

    def test_the_generated_baseline_is_what_compare_reads(self):
        # Belt and braces on the wiring: the file named on the compare line must be the
        # one the generate line produced.
        self.run_script(self.base_rev)
        _, cmp = self.calls()
        path = Path(cmp.split("--base-file ", 1)[1])
        self.assertEqual(json.loads(path.read_text()), {"stub": True})


if __name__ == "__main__":
    unittest.main()
