import io
import json
import os
import unittest
from contextlib import redirect_stdout
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
            self.t.latest_release = lambda repo: ("v0.2.0", "shiny")
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_update(ns(dir=None, check=True, ref=None))
            self.assertIn("0.2.0", buf.getvalue())
            self.assertIn("__version__ = '0.1.0'", p.read_text())  # unchanged

    def test_update_replaces_file_when_newer(self):
        with TemporaryDirectory() as tmp:
            p = self.make_self(tmp, "0.1.0")
            self.t.latest_release = lambda repo: ("v0.2.0", "notes")
            new_src = "#!/usr/bin/env python3\n__version__ = '0.2.0'\n# new\n"
            self.t.fetch_url = lambda url, accept=None: new_src
            with redirect_stdout(io.StringIO()):
                self.t.cmd_update(ns(dir=None, check=False, ref=None))
            self.assertEqual(p.read_text(), new_src)
            self.assertTrue(os.access(p, os.X_OK))  # exec bit preserved

    def test_already_up_to_date_is_noop(self):
        with TemporaryDirectory() as tmp:
            p = self.make_self(tmp, "0.2.0")
            self.t.latest_release = lambda repo: ("v0.2.0", "")
            before = p.read_text()
            with redirect_stdout(io.StringIO()):
                self.t.cmd_update(ns(dir=None, check=False, ref=None))
            self.assertEqual(p.read_text(), before)

    def test_compile_failure_does_not_overwrite(self):
        with TemporaryDirectory() as tmp:
            p = self.make_self(tmp, "0.1.0")
            self.t.latest_release = lambda repo: ("v0.2.0", "")
            self.t.fetch_url = lambda url, accept=None: "def (oops"  # invalid python
            with self.assertRaises(SystemExit):
                with redirect_stdout(io.StringIO()):
                    self.t.cmd_update(ns(dir=None, check=False, ref=None))
            self.assertIn("__version__ = '0.1.0'", p.read_text())  # untouched

    def test_network_error_aborts_cleanly(self):
        with TemporaryDirectory() as tmp:
            p = self.make_self(tmp, "0.1.0")
            def boom(repo): raise __import__("urllib").error.URLError("no net")
            self.t.latest_release = boom
            with self.assertRaises(SystemExit):
                with redirect_stdout(io.StringIO()):
                    self.t.cmd_update(ns(dir=None, check=False, ref=None))
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
            self.t.latest_release = lambda repo: ("v0.2.0", "")
            self.t.fetch_url = lambda url, accept=None: "def (oops"  # invalid python
            with self.assertRaises(SystemExit):
                with redirect_stdout(io.StringIO()):
                    self.t.cmd_update(ns(dir=None, check=False, ref=None))
            self.assertEqual(list(Path(tmp).glob("*.trck-update.tmp")), [])

    def test_replace_failure_cleans_up_temp_and_keeps_original(self):
        from unittest import mock
        with TemporaryDirectory() as tmp:
            p = self.make_self(tmp, "0.1.0")
            self.t.latest_release = lambda repo: ("v0.2.0", "")
            self.t.fetch_url = lambda url, accept=None: "#!/usr/bin/env python3\n__version__ = '0.2.0'\n"
            with mock.patch.object(self.t.os, "replace", side_effect=OSError("boom")):
                with self.assertRaises(SystemExit):
                    with redirect_stdout(io.StringIO()):
                        self.t.cmd_update(ns(dir=None, check=False, ref=None))
            self.assertEqual(list(Path(tmp).glob("*.trck-update.tmp")), [])  # cleaned up
            self.assertIn("__version__ = '0.1.0'", p.read_text())  # original intact
