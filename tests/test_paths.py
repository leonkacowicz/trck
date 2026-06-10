import io
import sys
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestPath(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title="Item", **over):
        a = dict(dir=str(d), title=title, priority="high", kind=None, parent=None,
                 points=None, depends=None, spec=None, slug=None)
        a.update(over)
        self.t.cmd_new(ns(**a))

    def cap(self, fn, args):
        buf = io.StringIO()
        with redirect_stdout(buf):
            fn(args)
        return buf.getvalue()

    def test_path_prints_absolute_path_for_id(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")                          # id 1
            out = self.cap(self.t.cmd_path, ns(dir=str(d), id=1)).strip()
            self.assertTrue(out.startswith("/"))
            self.assertTrue(out.endswith("001-alpha.md"))
            self.assertTrue(Path(out).is_file())

    def test_path_unknown_id_dies(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            with self.assertRaises(SystemExit):
                self.cap(self.t.cmd_path, ns(dir=str(d), id=99))


class TestWhich(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title="Item", **over):
        a = dict(dir=str(d), title=title, priority="high", kind=None, parent=None,
                 points=None, depends=None, spec=None, slug=None)
        a.update(over)
        self.t.cmd_new(ns(**a))

    def cap(self, fn, args):
        buf = io.StringIO()
        with redirect_stdout(buf):
            fn(args)
        return buf.getvalue()

    def which(self, d, paths, ids=False, stdin=None):
        a = ns(dir=str(d), paths=list(paths), ids=ids)
        if stdin is not None:
            real = sys.stdin
            sys.stdin = io.StringIO(stdin)
            try:
                return self.cap(self.t.cmd_which, a)
            finally:
                sys.stdin = real
        return self.cap(self.t.cmd_which, a)

    def cap_err(self, fn, args):
        buf = io.StringIO()
        with redirect_stderr(buf):
            fn(args)
        return buf.getvalue()

    def issue_file(self, d, issue_id):
        ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
        row = self.t.get_row(self.t.load_index(ctx), issue_id)
        return str(self.t.issue_path(ctx, row).resolve())

    def test_which_maps_path_to_list_row(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")                          # id 1
            p = self.issue_file(d, 1)
            out = self.which(d, [p])
            self.assertIn("#001", out)
            self.assertIn("Alpha", out)

    def test_which_ids_flag_prints_bare_id(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")                          # id 1
            p = self.issue_file(d, 1)
            out = self.which(d, [p], ids=True).strip()
            self.assertEqual(out, "1")

    def test_which_reads_paths_from_stdin(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")                          # id 1
            self.seed(d, "Beta")                           # id 2
            piped = self.issue_file(d, 1) + "\n" + self.issue_file(d, 2) + "\n"
            out = self.which(d, [], stdin=piped)
            self.assertIn("#001", out)
            self.assertIn("#002", out)

    def test_which_bare_filename_resolves_by_leading_id(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")                          # id 1 -> 001-alpha.md
            out = self.which(d, ["issues/backlog/001-alpha.md"])
            self.assertIn("#001", out)

    def test_which_skips_non_issue_path(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")                          # id 1
            out = self.which(d, [self.issue_file(d, 1), "issues/SUMMARY.md"])
            self.assertIn("#001", out)                     # known one still rendered
            self.assertNotIn("SUMMARY", out)               # junk path dropped from stdout

    def test_which_unknown_id_is_skipped(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")                          # only id 1 exists
            out = self.which(d, ["someplace/777-ghost.md"])
            self.assertEqual(out.strip(), "")              # no row, no crash

    def test_which_dedupes_and_orders_by_id(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")                          # 1
            self.seed(d, "Beta")                           # 2
            ps = [self.issue_file(d, 2), self.issue_file(d, 1), self.issue_file(d, 2)]
            out = self.which(d, ps, ids=True)
            self.assertEqual(out.split(), ["1", "2"])

    def test_which_warns_on_non_issue_path(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")                          # id 1
            args = ns(dir=str(d), paths=["issues/SUMMARY.md"], ids=False)
            err = self.cap_err(self.t.cmd_which, args)
            self.assertIn("warning:", err)
            self.assertIn("SUMMARY.md", err)

    def test_list_paths_round_trips_through_which(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")                          # 1
            self.seed(d, "Beta")                           # 2
            # capture `list --paths` output, then feed it straight into `which`
            list_args = ns(dir=str(d), status=None, kind=None, priority=None, label=None,
                           parent=None, match=None, sort=None, blocked=False, orphan=False,
                           flat=False, id=None, paths=True)
            paths_out = self.cap(self.t.cmd_list, list_args)
            self.assertEqual(len(paths_out.splitlines()), 2)
            out = self.which(d, paths_out.splitlines())
            self.assertIn("#001", out)
            self.assertIn("#002", out)
            self.assertIn("Alpha", out)
            self.assertIn("Beta", out)


class TestWiring(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_list_paths_flag_parses(self):
        a = self.t.build_parser().parse_args(["list", "--paths", "--status", "ongoing"])
        self.assertTrue(a.paths)
        self.assertEqual(a.status, "ongoing")
        self.assertIs(a.func, self.t.cmd_list)

    def test_path_verb_parses_id(self):
        a = self.t.build_parser().parse_args(["path", "7"])
        self.assertEqual(a.id, 7)
        self.assertIs(a.func, self.t.cmd_path)

    def test_which_verb_parses_paths_and_ids(self):
        a = self.t.build_parser().parse_args(["which", "--ids", "a.md", "b.md"])
        self.assertTrue(a.ids)
        self.assertEqual(a.paths, ["a.md", "b.md"])
        self.assertIs(a.func, self.t.cmd_which)

    def test_which_defaults_to_no_paths_for_stdin(self):
        a = self.t.build_parser().parse_args(["which"])
        self.assertEqual(a.paths, [])
        self.assertFalse(a.ids)
