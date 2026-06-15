import subprocess
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns, TRCK_PATH


class TestIdArgs(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_id_args_accept_string_tokens(self):
        p = self.t.build_parser()
        args = p.parse_args(["show", "k3m9x2a"])
        self.assertEqual(args.id, "k3m9x2a")

    def test_get_row_coerces_int_token(self):
        t = self.t
        row = t.Issue(id="5", slug="s", title="T", kind="task",
                      status="backlog", priority="high")
        self.assertIs(t.get_row([row], 5), row)    # int token still resolves
        self.assertIs(t.get_row([row], "5"), row)


class TestAliases(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d):
        self.t.cmd_new(ns(dir=str(d), title="Item", priority="high", kind=None,
                          parent=None, depends=None, spec=None, slug=None))

    def rows(self, d):
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return {r.id: r for r in self.t.load_index(ctx)}

    def test_start_alias_moves_to_ongoing(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.t.cmd_start(ns(dir=str(d), id=1))
            self.assertEqual(self.rows(d)["1"].status, "ongoing")

    def test_done_alias_with_resolution(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.t.cmd_done(ns(dir=str(d), id=1, resolution="duplicate"))
            r = self.rows(d)["1"]
            self.assertEqual(r.status, "done")
            self.assertEqual(r.resolution, "duplicate")

    def test_undefined_alias_dies(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {"aliases": {}})  # no start alias
            self.seed(d)
            with self.assertRaises(SystemExit):
                self.t.cmd_start(ns(dir=str(d), id=1))


class TestDie(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_die_raises_and_never_returns(self):
        with self.assertRaises(SystemExit) as cm:
            self.t.die("boom")
        self.assertEqual(cm.exception.code, 1)

    def test_die_annotated_noreturn(self):
        # Locks in the NoReturn annotation so callers' type-narrowing
        # (some_var bound after `except: die()`) stays valid.
        import typing
        hints = typing.get_type_hints(self.t.die)
        self.assertIs(hints["return"], typing.NoReturn)


class TestCliEndToEnd(unittest.TestCase):
    def run_trck(self, *argv, cwd):
        return subprocess.run(["python3", str(TRCK_PATH), *argv],
                              cwd=cwd, capture_output=True, text=True)

    def test_version_subcommand(self):
        with TemporaryDirectory() as tmp:
            r = self.run_trck("version", cwd=tmp)
            self.assertEqual(r.returncode, 0)
            self.assertRegex(r.stdout.strip().splitlines()[0], r"\d+\.\d+\.\d+")
            # outside any tracker: no error/noise on stderr
            self.assertNotIn("no tracker", r.stderr)
            self.assertNotIn("error:", r.stderr)

    def test_new_then_list_via_cli(self):
        with TemporaryDirectory() as tmp:
            make_tracker(tmp, {})
            self.run_trck("--dir", str(Path(tmp) / "issues"), "new", "Hello", cwd=tmp)
            r = self.run_trck("--dir", str(Path(tmp) / "issues"), "list", cwd=tmp)
            self.assertIn("Hello", r.stdout)
