import io
import json
import os
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestVersionAndFetch(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_parse_version_orders_correctly(self):
        pv = self.t.parse_version
        self.assertEqual(pv("v0.10.1"), (0, 10, 1))
        self.assertTrue(pv("0.10.0") > pv("0.9.9"))
        self.assertTrue(pv("1.0.0") > pv("0.99.0"))

    def test_latest_release_parses_tag_and_body(self):
        payload = json.dumps({"tag_name": "v1.2.3", "body": "notes here"})
        self.t.fetch_url = lambda url, accept=None: payload  # monkeypatch the seam
        tag, body = self.t.latest_release("owner/repo")
        self.assertEqual(tag, "v1.2.3")
        self.assertEqual(body, "notes here")


class TestUpdate(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def make_self(self, tmp, version):
        """Write a fake engine file and point SELF_PATH at it (never the real ./trck)."""
        p = Path(tmp) / "trck"
        p.write_text(f"#!/usr/bin/env python3\n__version__ = '{version}'\n")
        p.chmod(0o755)
        self.t.SELF_PATH = p.resolve()
        return p

    def test_check_reports_newer_without_writing(self):
        with TemporaryDirectory() as tmp:
            p = self.make_self(tmp, "0.1.0")
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_update(ns(dir=None, check=True, ref="v0.2.0"))
            self.assertIn("0.2.0", buf.getvalue())
            self.assertIn("__version__ = '0.1.0'", p.read_text())  # unchanged

    def test_update_replaces_file_when_newer(self):
        with TemporaryDirectory() as tmp:
            p = self.make_self(tmp, "0.1.0")
            new_src = "#!/usr/bin/env python3\n__version__ = '0.2.0'\n# new\n"
            self.t.fetch_url = lambda url, accept=None: new_src
            with redirect_stdout(io.StringIO()):
                self.t.cmd_update(ns(dir=None, check=False, ref="v0.2.0"))
            self.assertEqual(p.read_text(), new_src)
            self.assertTrue(os.access(p, os.X_OK))  # exec bit preserved

    # --- the end of the line ---------------------------------------------------- #

    def test_update_without_a_ref_names_the_migration_instead_of_fetching(self):
        """This is the last Python engine, so there is nothing left to update to.

        The alternative was to let it keep fetching: once `./trck` leaves the tree, every
        engine in the wild would ask for a path that 404s and report the network as the
        cause of a decision this project made. Saying so plainly is the whole job of the
        final release."""
        with TemporaryDirectory() as tmp:
            p = self.make_self(tmp, "0.25.1")
            before = p.read_text()

            def unreachable(*a, **k):
                raise AssertionError("update went to the network after the final release")

            self.t.latest_release = unreachable
            self.t.fetch_url = unreachable
            buf = io.StringIO()
            with self.assertRaises(SystemExit) as cm, redirect_stdout(buf):
                self.t.cmd_update(ns(dir=None, check=False, ref=None))
            self.assertNotEqual(cm.exception.code, 0, "a dead update path exited 0")
            self.assertEqual(p.read_text(), before, "the engine was replaced")

    def test_the_notice_says_how_to_get_the_binary(self):
        with TemporaryDirectory() as tmp:
            self.make_self(tmp, "0.25.1")
            err = io.StringIO()
            with self.assertRaises(SystemExit), redirect_stderr(err):
                self.t.cmd_update(ns(dir=None, check=False, ref=None))
            text = err.getvalue()
            self.assertIn("install.sh", text, text)
            self.assertIn("--ref", text, text)  # the escape hatch is named

    def test_check_also_reports_the_end_of_the_line(self):
        """`--check` asks whether an update is available. The answer is no, and why."""
        with TemporaryDirectory() as tmp:
            self.make_self(tmp, "0.25.1")

            def unreachable(*a, **k):
                raise AssertionError("--check went to the network")

            self.t.latest_release = unreachable
            with self.assertRaises(SystemExit), redirect_stderr(io.StringIO()):
                self.t.cmd_update(ns(dir=None, check=True, ref=None))

    def test_compile_failure_does_not_overwrite(self):
        with TemporaryDirectory() as tmp:
            p = self.make_self(tmp, "0.1.0")
            self.t.fetch_url = lambda url, accept=None: "def (oops"  # invalid python
            with self.assertRaises(SystemExit), redirect_stdout(io.StringIO()):
                self.t.cmd_update(ns(dir=None, check=False, ref="v0.2.0"))
            self.assertIn("__version__ = '0.1.0'", p.read_text())  # untouched

    def test_network_error_aborts_cleanly(self):
        with TemporaryDirectory() as tmp:
            p = self.make_self(tmp, "0.1.0")
            def boom(url, accept=None): raise __import__("urllib").error.URLError("no net")
            self.t.fetch_url = boom
            with self.assertRaises(SystemExit), redirect_stdout(io.StringIO()):
                self.t.cmd_update(ns(dir=None, check=False, ref="v0.2.0"))
            self.assertIn("__version__ = '0.1.0'", p.read_text())

    def test_ref_writes_regardless_of_version(self):
        with TemporaryDirectory() as tmp:
            p = self.make_self(tmp, "0.2.0")  # same version as we'll write
            new_src = "#!/usr/bin/env python3\n__version__ = '0.2.0'\n# via ref\n"
            self.t.fetch_url = lambda url, accept=None: new_src
            with redirect_stdout(io.StringIO()):
                self.t.cmd_update(ns(dir=None, check=False, ref="some-branch"))
            self.assertEqual(p.read_text(), new_src)  # --ref skips version compare

    def test_compile_failure_leaves_no_temp_file(self):
        with TemporaryDirectory() as tmp:
            self.make_self(tmp, "0.1.0")
            self.t.fetch_url = lambda url, accept=None: "def (oops"  # invalid python
            with self.assertRaises(SystemExit), redirect_stdout(io.StringIO()):
                self.t.cmd_update(ns(dir=None, check=False, ref="v0.2.0"))
            self.assertEqual(list(Path(tmp).glob("*.trck-update.tmp")), [])

    def test_replace_failure_cleans_up_temp_and_keeps_original(self):
        from unittest import mock
        with TemporaryDirectory() as tmp:
            p = self.make_self(tmp, "0.1.0")
            self.t.fetch_url = lambda url, accept=None: "#!/usr/bin/env python3\n__version__ = '0.2.0'\n"
            with mock.patch.object(self.t.os, "replace", side_effect=OSError("boom")), \
                    self.assertRaises(SystemExit), redirect_stdout(io.StringIO()):
                self.t.cmd_update(ns(dir=None, check=False, ref="v0.2.0"))
            self.assertEqual(list(Path(tmp).glob("*.trck-update.tmp")), [])  # cleaned up
            self.assertIn("__version__ = '0.1.0'", p.read_text())  # original intact


class TestUpdateRefreshesManagedDocs(unittest.TestCase):
    """`trck update` also refreshes scaffolded docs (CLAUDE.md) the user hasn't edited."""

    def setUp(self):
        self.t = load_trck()

    def make_self(self, tmp, version):
        p = Path(tmp) / "trck"
        p.write_text(f"#!/usr/bin/env python3\n__version__ = '{version}'\n")
        p.chmod(0o755)
        self.t.SELF_PATH = p.resolve()
        return p

    def new_source(self, version, claude_template):
        """A minimal but valid engine source carrying a CLAUDE_MD_TEMPLATE literal."""
        return (
            "#!/usr/bin/env python3\n"
            f"__version__ = '{version}'\n"
            f"CLAUDE_MD_TEMPLATE = {claude_template!r}\n"
        )

    def run_update(self, tracker, source):
        # Through `--ref`: the version-comparing path is retired with the engine, and this
        # is about what happens *after* a fetch succeeds, which `--ref` still reaches.
        self.t.fetch_url = lambda url, accept=None: source
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_update(ns(dir=str(tracker), check=False, ref="v0.2.0"))
        return buf.getvalue()

    def test_refreshes_when_doc_matches_current_template(self):
        with TemporaryDirectory() as tmp:
            self.make_self(tmp, "0.1.0")
            tracker = make_tracker(tmp)
            self.t.CLAUDE_MD_TEMPLATE = "OLD GUIDE\n"
            (tracker / "CLAUDE.md").write_text("OLD GUIDE\n")  # untouched by user
            out = self.run_update(tracker, self.new_source("0.2.0", "NEW GUIDE\n"))
            self.assertEqual((tracker / "CLAUDE.md").read_text(), "NEW GUIDE\n")
            self.assertIn("refreshed", out)

    def test_keeps_doc_when_user_modified_it(self):
        with TemporaryDirectory() as tmp:
            self.make_self(tmp, "0.1.0")
            tracker = make_tracker(tmp)
            self.t.CLAUDE_MD_TEMPLATE = "OLD GUIDE\n"
            (tracker / "CLAUDE.md").write_text("MY CUSTOM EDITS\n")  # diverged from template
            out = self.run_update(tracker, self.new_source("0.2.0", "NEW GUIDE\n"))
            self.assertEqual((tracker / "CLAUDE.md").read_text(), "MY CUSTOM EDITS\n")
            self.assertIn("kept your modified", out)

    def test_no_write_when_template_unchanged(self):
        with TemporaryDirectory() as tmp:
            self.make_self(tmp, "0.1.0")
            tracker = make_tracker(tmp)
            self.t.CLAUDE_MD_TEMPLATE = "SAME GUIDE\n"
            (tracker / "CLAUDE.md").write_text("SAME GUIDE\n")
            out = self.run_update(tracker, self.new_source("0.2.0", "SAME GUIDE\n"))
            self.assertEqual((tracker / "CLAUDE.md").read_text(), "SAME GUIDE\n")
            self.assertNotIn("refreshed", out)

    def test_missing_doc_is_noop(self):
        with TemporaryDirectory() as tmp:
            self.make_self(tmp, "0.1.0")
            tracker = make_tracker(tmp)  # no CLAUDE.md scaffolded
            self.t.CLAUDE_MD_TEMPLATE = "OLD GUIDE\n"
            self.run_update(tracker, self.new_source("0.2.0", "NEW GUIDE\n"))
            self.assertFalse((tracker / "CLAUDE.md").exists())  # not created

    def test_new_engine_without_template_is_noop(self):
        with TemporaryDirectory() as tmp:
            self.make_self(tmp, "0.1.0")
            tracker = make_tracker(tmp)
            self.t.CLAUDE_MD_TEMPLATE = "OLD GUIDE\n"
            (tracker / "CLAUDE.md").write_text("OLD GUIDE\n")
            # downloaded engine carries no CLAUDE_MD_TEMPLATE literal -> leave doc alone
            self.run_update(tracker, "#!/usr/bin/env python3\n__version__ = '0.2.0'\n")
            self.assertEqual((tracker / "CLAUDE.md").read_text(), "OLD GUIDE\n")
