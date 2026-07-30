"""`trck repo setup-git` (#7ed853j): declare the drivers and register them.

The crux this verb exists for: `.gitattributes` is committed and shared, but it can
only *name* a merge driver — the `driver = …` command lives in `.git/config`, which
git deliberately never shares (otherwise cloning a repo would be remote code
execution). So the drivers do nothing until each clone registers them locally.
"""
import subprocess
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, ns


class TestSetupGit(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def repo(self, tmp):
        """A git repo with a tracker at issues/."""
        root = Path(tmp) / "repo"
        (root / "issues").mkdir(parents=True)
        (root / "issues" / "trck.json").write_text("{}")
        (root / "issues" / "index.jsonl").write_text("")
        for cmd in (["init", "-q", "-b", "main"], ["config", "user.email", "t@t"],
                    ["config", "user.name", "t"]):
            subprocess.run(["git", *cmd], cwd=root, check=True,
                           capture_output=True)
        return root

    def run_setup(self, root, **over):
        args = dict(dir=str(root / "issues"))
        args.update(over)
        self.t.cmd_setup_git(ns(**args))

    def git_config(self, root, key):
        r = subprocess.run(["git", "config", "--get", key], cwd=root,
                           capture_output=True, text=True)
        return r.stdout.strip()

    # --- the shared half: .gitattributes -------------------------------------- #

    def test_writes_gitattributes_naming_both_drivers(self):
        with TemporaryDirectory() as tmp:
            root = self.repo(tmp)
            self.run_setup(root)
            text = (root / "issues" / ".gitattributes").read_text()
            self.assertIn("index.jsonl merge=trck-index", text)
            self.assertIn("SUMMARY.md merge=trck-summary", text)

    def test_gitattributes_is_idempotent(self):
        with TemporaryDirectory() as tmp:
            root = self.repo(tmp)
            self.run_setup(root)
            first = (root / "issues" / ".gitattributes").read_text()
            self.run_setup(root)
            self.assertEqual((root / "issues" / ".gitattributes").read_text(), first)

    def test_existing_gitattributes_content_is_preserved(self):
        with TemporaryDirectory() as tmp:
            root = self.repo(tmp)
            (root / "issues" / ".gitattributes").write_text("*.png binary\n")
            self.run_setup(root)
            text = (root / "issues" / ".gitattributes").read_text()
            self.assertIn("*.png binary", text)
            self.assertIn("merge=trck-index", text)

    # --- the per-clone half: .git/config -------------------------------------- #

    def test_registers_both_drivers_in_git_config(self):
        with TemporaryDirectory() as tmp:
            root = self.repo(tmp)
            self.run_setup(root)
            self.assertIn("merge-index", self.git_config(root, "merge.trck-index.driver"))
            self.assertIn("merge-summary", self.git_config(root, "merge.trck-summary.driver"))

    def test_driver_command_passes_gits_three_operands(self):
        with TemporaryDirectory() as tmp:
            root = self.repo(tmp)
            self.run_setup(root)
            cmd = self.git_config(root, "merge.trck-index.driver")
            for token in ("%O", "%A", "%B"):
                self.assertIn(token, cmd)

    def test_registration_is_idempotent(self):
        with TemporaryDirectory() as tmp:
            root = self.repo(tmp)
            self.run_setup(root)
            once = self.git_config(root, "merge.trck-index.driver")
            self.run_setup(root)
            self.assertEqual(self.git_config(root, "merge.trck-index.driver"), once)

    def test_driver_names_are_set_for_conflict_reporting(self):
        with TemporaryDirectory() as tmp:
            root = self.repo(tmp)
            self.run_setup(root)
            self.assertTrue(self.git_config(root, "merge.trck-index.name"))

    # --- failure modes --------------------------------------------------------- #

    def test_outside_a_git_repo_it_dies(self):
        with TemporaryDirectory() as tmp:
            d = Path(tmp) / "loose"
            (d).mkdir()
            (d / "trck.json").write_text("{}")
            with self.assertRaises(SystemExit):
                self.t.cmd_setup_git(ns(dir=str(d)))
