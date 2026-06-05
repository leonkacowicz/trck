import io
import json
import os
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, ns


class TestInit(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def make_self(self, tmp):
        """Point SELF_PATH at a throwaway engine copy so init vendors that, not real ./trck."""
        p = Path(tmp) / "trck"
        p.write_text("#!/usr/bin/env python3\n__version__='9.9.9'\n")
        p.chmod(0o755)
        self.t.SELF_PATH = p.resolve()
        return p

    def init(self, repo, **over):
        a = ns(target=None, init_dir=None, no_vendor=False, hook=False, force=False)
        a.cwd = str(repo)
        for k, v in over.items():
            setattr(a, k, v)
        # cmd_init resolves the target relative to cwd; emulate by chdir
        old = os.getcwd()
        os.chdir(repo)
        try:
            with redirect_stdout(io.StringIO()):
                self.t.cmd_init(a)
        finally:
            os.chdir(old)

    def test_init_creates_structure(self):
        with TemporaryDirectory() as tmp:
            self.make_self(tmp)
            repo = Path(tmp) / "repo"; repo.mkdir()
            self.init(repo)
            d = repo / "issues"
            self.assertTrue((d / "trck.json").exists())
            self.assertTrue((d / "trck").exists())
            self.assertTrue(os.access(d / "trck", os.X_OK))
            self.assertTrue((d / "CLAUDE.md").exists())
            cfg = json.loads((d / "trck.json").read_text())
            self.assertEqual(cfg["update"]["repo"], self.t.DEFAULT_UPDATE_REPO)
            self.assertEqual([s["name"] for s in cfg["statuses"]],
                             ["backlog", "ongoing", "done"])

    def test_init_refuses_existing_without_force(self):
        with TemporaryDirectory() as tmp:
            self.make_self(tmp)
            repo = Path(tmp) / "repo"; repo.mkdir()
            self.init(repo)
            with self.assertRaises(SystemExit):
                self.init(repo)            # second time, no force
            self.init(repo, force=True)    # force overwrites config OK (no raise)

    def test_init_then_new_works(self):
        with TemporaryDirectory() as tmp:
            self.make_self(tmp)
            repo = Path(tmp) / "repo"; repo.mkdir()
            self.init(repo)
            d = repo / "issues"
            self.t.cmd_new(ns(dir=str(d), title="X", priority="high", epic=False,
                              parent=None, milestone=None, depends=None,
                              spec=None, slug=None))
            self.assertTrue((d / "backlog" / "001-x.md").exists())

    def test_init_positional_dir(self):
        with TemporaryDirectory() as tmp:
            self.make_self(tmp)
            repo = Path(tmp) / "repo"; repo.mkdir()
            self.init(repo, target="tracker")   # positional dir instead of --dir
            d = repo / "tracker"
            self.assertTrue((d / "trck.json").exists())
            self.assertTrue((d / "CLAUDE.md").exists())
            self.assertTrue((d / "README.md").exists())
            self.assertTrue((d / "trck").exists())   # vendored by default

    def test_init_rejects_positional_and_dir_together(self):
        with TemporaryDirectory() as tmp:
            self.make_self(tmp)
            repo = Path(tmp) / "repo"; repo.mkdir()
            with self.assertRaises(SystemExit):
                self.init(repo, target="a", init_dir="b")

    def test_init_no_vendor_skips_engine_copy(self):
        with TemporaryDirectory() as tmp:
            self.make_self(tmp)
            repo = Path(tmp) / "repo"; repo.mkdir()
            self.init(repo, no_vendor=True)
            d = repo / "issues"
            self.assertTrue((d / "trck.json").exists())
            self.assertTrue((d / "CLAUDE.md").exists())
            self.assertFalse((d / "trck").exists())   # engine NOT vendored

    def test_init_refuses_vendor_over_running_engine(self):
        with TemporaryDirectory() as tmp:
            self.make_self(tmp)   # SELF_PATH = tmp/trck
            # init targeting the engine's own dir would vendor over itself -> clean die
            with self.assertRaises(SystemExit):
                self.init(Path(tmp), target=".")
            # ...but --no-vendor makes it fine
            self.init(Path(tmp), target=".", no_vendor=True)
            self.assertTrue((Path(tmp) / "trck.json").exists())
