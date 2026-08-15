"""Tests for scripts/hooks/pre-commit — the developer's local guard.

The hook is a convenience, so the failure that matters is not "it missed something" but
"it refused a commit it had no business refusing". It used to run `trck check`, which made
every commit in this repo depend on a tracker directory being present and consistent — and
the commit that moves the tracker onto its own ref is precisely the one that removes that
directory. So these tests assert an absence: whatever else the hook does, it does not
consult the tracker.

The hook is run directly rather than through `git commit`, so a failure reports the hook's
own output instead of git's summary of it.

    python3 -m unittest discover -s scripts/tests
"""
import os
import shutil
import subprocess
import tempfile
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
HOOK = REPO_ROOT / "scripts" / "hooks" / "pre-commit"


def tool(name):
    """An absolute path to `name`, or skip the module — the hook cannot run without it."""
    found = shutil.which(name)
    if found is None:
        raise unittest.SkipTest(f"the pre-commit hook needs {name}")
    return found


BASH = tool("bash")
GIT = tool("git")


def fake(directory, name, body):
    """Put an executable `name` on a PATH directory, and return where it records its calls."""
    log = directory / f"{name}.calls"
    path = directory / name
    path.write_text(f'#!/bin/sh\necho "$@" >> "{log}"\n{body}\n')
    path.chmod(0o755)
    return log


class Harness(unittest.TestCase):
    """A throwaway git repo with a controlled PATH, so only the fakes are reachable."""

    def setUp(self):
        self._tmp = tempfile.TemporaryDirectory()
        self.root = Path(self._tmp.name) / "repo"
        (self.root / "bin").mkdir(parents=True)
        subprocess.run(["git", "init", "-q"], cwd=self.root, check=True)
        self.bin = self.root / "bin"

    def tearDown(self):
        self._tmp.cleanup()

    def run_hook(self):
        """The hook, on a PATH holding the fakes and git and nothing else.

        Not a prefix of the developer's PATH: the machine running this has a real ratchet
        and a real trck on it, and "the tool is absent" is one of the cases under test.
        """
        env = dict(os.environ, PATH=f"{self.bin}{os.pathsep}{Path(GIT).parent}")
        return subprocess.run(
            [BASH, str(HOOK)], cwd=self.root, env=env, capture_output=True, text=True,
        )


class TrackerIsNotConsulted(Harness):
    """The tracker lives on its own ref; a commit in this tree cannot affect it."""

    def test_does_not_run_trck_even_when_one_is_on_path(self):
        calls = fake(self.bin, "trck", "exit 0")
        self.run_hook()
        self.assertFalse(calls.exists(), f"hook invoked trck: {calls.read_text() if calls.exists() else ''}")

    def test_a_failing_trck_does_not_fail_the_commit(self):
        fake(self.bin, "trck", "exit 1")
        result = self.run_hook()
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_does_not_run_a_trck_built_in_the_tree(self):
        """`target/release/trck` used to be preferred over an installed one. Neither is used."""
        (self.root / "target" / "release").mkdir(parents=True)
        calls = fake(self.root / "target" / "release", "trck", "exit 1")
        result = self.run_hook()
        self.assertFalse(calls.exists(), "hook invoked the built engine")
        self.assertEqual(result.returncode, 0, result.stderr)

    def test_says_nothing_about_a_missing_trck(self):
        """Absent trck used to print two lines of advice. There is nothing to advise now."""
        fake(self.bin, "ratchet", "exit 0")
        result = self.run_hook()
        self.assertNotIn("trck", result.stderr)
        self.assertNotIn("trck", result.stdout)


class QualityReport(Harness):
    """The one check the hook still makes."""

    def test_runs_ratchet_check(self):
        calls = fake(self.bin, "ratchet", "exit 0")
        result = self.run_hook()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("check", calls.read_text())

    def test_a_stale_report_fails_the_commit(self):
        fake(self.bin, "ratchet", "exit 1")
        result = self.run_hook()
        self.assertEqual(result.returncode, 1)
        self.assertIn("ratchet generate", result.stderr)

    def test_a_missing_ratchet_skips_rather_than_fails(self):
        result = self.run_hook()
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("no ratchet found", result.stderr)


if __name__ == "__main__":
    unittest.main()
