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

    # --- the line ending is part of the format --------------------------------- #

    def test_everything_the_engine_writes_is_pinned_to_lf(self):
        """A CRLF checkout would put the working tree at odds with the engine.

        `index.jsonl` and `SUMMARY.md` are compared byte for byte and rendered
        with `\\n`; the bodies are rewritten by `edit --title`. Clone any of them
        onto a machine with `core.autocrlf=true` and the next verb rewrites the
        whole file back, so every commit shows it as wholly changed."""
        with TemporaryDirectory() as tmp:
            root = self.repo(tmp)
            self.run_setup(root)
            text = (root / "issues" / ".gitattributes").read_text()
            for pattern in ("index.jsonl", "SUMMARY.md", "items/*.md"):
                line = next(ln for ln in text.splitlines()
                            if ln.split() and ln.split()[0] == pattern)
                self.assertIn("text", line.split(), line)
                self.assertIn("eol=lf", line.split(), line)

    def test_a_tracker_set_up_before_the_pin_is_upgraded_in_place(self):
        """The old line is replaced, not joined by a second one for the same path.

        Two lines naming `index.jsonl` would in fact work — git applies the last
        value for each attribute — but a managed block that grows a stale copy of
        itself on every upgrade is one nobody can read."""
        with TemporaryDirectory() as tmp:
            root = self.repo(tmp)
            (root / "issues" / ".gitattributes").write_text(
                f"{self.t.GITATTRIBUTES_HEADER}\n"
                "index.jsonl merge=trck-index\n"
                "SUMMARY.md merge=trck-summary\n")
            self.run_setup(root)
            lines = (root / "issues" / ".gitattributes").read_text().splitlines()
            naming_index = [ln for ln in lines
                            if ln.split() and ln.split()[0] == "index.jsonl"]
            self.assertEqual(len(naming_index), 1, lines)
            self.assertIn("eol=lf", naming_index[0].split())

    def test_a_users_own_rule_for_our_path_is_not_overwritten(self):
        """Replacing in place is for *our* stale lines. A rule carrying anything
        we do not manage is someone's decision, so ours is added beside it and
        git resolves the two."""
        with TemporaryDirectory() as tmp:
            root = self.repo(tmp)
            (root / "issues" / ".gitattributes").write_text("index.jsonl -diff\n")
            self.run_setup(root)
            lines = (root / "issues" / ".gitattributes").read_text().splitlines()
            self.assertIn("index.jsonl -diff", lines)
            self.assertTrue(any("merge=trck-index" in ln for ln in lines), lines)

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

    # --- which engine the driver re-invokes ------------------------------------ #

    def test_driver_prefers_a_vendored_engine(self):
        """A copy committed beside the tracker is pinned to the data's version."""
        with TemporaryDirectory() as tmp:
            root = self.repo(tmp)
            vendored = root / "issues" / "trck"
            vendored.write_text("#!/usr/bin/env python3\n")
            self.run_setup(root)
            self.assertIn(str(vendored.resolve()),
                          self.git_config(root, "merge.trck-index.driver"))

    def test_driver_falls_back_to_the_running_engine_not_the_path(self):
        """With no vendored copy, re-invoke the engine file running right now.

        A bare `trck` would make the driver depend on an install that need not
        exist (CI checkouts have none) and, where it does, need not be this
        engine or this version."""
        with TemporaryDirectory() as tmp:
            root = self.repo(tmp)
            self.run_setup(root)
            cmd = self.git_config(root, "merge.trck-index.driver")
            self.assertIn(str(Path(self.t.SELF_PATH).resolve()), cmd)
            self.assertFalse(cmd.startswith("trck "), f"driver leans on PATH: {cmd}")

    # --- failure modes --------------------------------------------------------- #

    def test_outside_a_git_repo_it_dies(self):
        with TemporaryDirectory() as tmp:
            d = Path(tmp) / "loose"
            (d).mkdir()
            (d / "trck.json").write_text("{}")
            with self.assertRaises(SystemExit):
                self.t.cmd_setup_git(ns(dir=str(d)))
