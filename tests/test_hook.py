import subprocess
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, ns

# A stub engine: when the hook runs `python3 <path> check`, it drops a sentinel
# file in the cwd so the test can observe that the hook actually fired.
STUB = "import pathlib; pathlib.Path('SENTINEL').write_text('ran')\n"


class TestInstallHook(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def git(self, repo, *args):
        subprocess.run(["git", *args], cwd=repo, check=True,
                       capture_output=True, text=True)

    def make_repo(self, tmp):
        repo = Path(tmp)
        self.git(repo, "init", "-q")
        return repo

    def stub_engine(self, at):
        at.write_text(STUB)
        at.chmod(0o755)

    def run_hook(self, repo):
        """Run the installed pre-commit hook the way git would; return its rc."""
        return subprocess.run(["bash", str(repo / ".git" / "hooks" / "pre-commit")],
                              cwd=repo, capture_output=True, text=True).returncode

    def test_fires_when_tracker_is_repo_root(self):
        with TemporaryDirectory() as tmp:
            repo = self.make_repo(tmp)
            (repo / "trck.json").write_text("{}")
            self.stub_engine(repo / "trck")
            self.t.cmd_install_hook(ns(dir=str(repo)))

            (repo / "index.jsonl").write_text("")
            self.git(repo, "add", "index.jsonl")
            self.run_hook(repo)

            self.assertTrue((repo / "SENTINEL").exists(),
                            "hook must run check when the whole repo is the tracker")

    def test_fires_when_nested_tracker_files_staged(self):
        with TemporaryDirectory() as tmp:
            repo = self.make_repo(tmp)
            tracker = repo / "issues"; tracker.mkdir()
            (tracker / "trck.json").write_text("{}")
            self.stub_engine(tracker / "trck")
            self.t.cmd_install_hook(ns(dir=str(tracker)))

            (tracker / "index.jsonl").write_text("")
            self.git(repo, "add", "issues/index.jsonl")
            self.run_hook(repo)

            self.assertTrue((repo / "SENTINEL").exists(),
                            "hook must run check when a tracker file is staged")

    def test_skips_when_nested_tracker_untouched(self):
        with TemporaryDirectory() as tmp:
            repo = self.make_repo(tmp)
            tracker = repo / "issues"; tracker.mkdir()
            (tracker / "trck.json").write_text("{}")
            self.stub_engine(tracker / "trck")
            self.t.cmd_install_hook(ns(dir=str(tracker)))

            (repo / "README.md").write_text("hi")
            self.git(repo, "add", "README.md")
            self.run_hook(repo)

            self.assertFalse((repo / "SENTINEL").exists(),
                             "hook must not fire when no tracker file is staged")


if __name__ == "__main__":
    unittest.main()
