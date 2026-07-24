"""Tests for the standalone accessory `tools/trck-html` (v1 MVP).

The tool loads the generated `./trck` engine at runtime and exposes a pure
`render_html(ctx) -> str` that emits one self-contained HTML SPA whose data lives
in an embedded JSON island. These tests drive that core: they seed a temp tracker
with the engine, then assert on the string the tool produces (no browser needed).
"""
import importlib.machinery
import importlib.util
import io
import json
import os
import re
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns

REPO_ROOT = Path(__file__).resolve().parent.parent
TOOL_PATH = REPO_ROOT / "tools" / "trck-html"


def load_tool():
    """Import the extensionless `tools/trck-html` as a fresh module object.

    Point it at this repo's freshly-built `./trck` so engine discovery is
    deterministic regardless of the test runner's cwd."""
    os.environ["TRCK_ENGINE"] = str(REPO_ROOT / "trck")
    loader = importlib.machinery.SourceFileLoader("trck_html_tool", str(TOOL_PATH))
    spec = importlib.util.spec_from_file_location("trck_html_tool", TOOL_PATH, loader=loader)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["trck_html_tool"] = mod
    spec.loader.exec_module(mod)
    return mod


def island(html):
    """Parse the embedded JSON data island out of a rendered document. The
    non-greedy match stops at the FIRST `</script>`; if an issue body smuggled an
    unescaped `</script>` in, this truncates and json.loads blows up — which is
    exactly the escaping regression we want to catch."""
    m = re.search(r'<script[^>]*id="trck-data"[^>]*>(.*?)</script>', html, re.S)
    assert m, "no trck-data island found"
    return json.loads(m.group(1))


class HtmlTestBase(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.h = load_tool()

    def seed(self, d, title="Item", **over):
        a = dict(dir=str(d), title=title, priority="medium", kind=None, parent=None,
                 points=None, depends=None, spec=None, slug=None)
        a.update(over)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(ns(**a))
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def body_file(self, d, iid):
        hits = list(Path(d).glob(f"*/{iid}-*.md"))
        assert len(hits) == 1, f"expected one file for {iid}, got {hits}"
        return hits[0]

    def render(self, d):
        ctx = self.h.build_ctx_from(str(d))
        return self.h.render_html(ctx)


class TestRenderShell(HtmlTestBase):
    def test_output_is_a_full_html_document(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            html = self.render(d)
            self.assertTrue(html.lstrip().lower().startswith("<!doctype html"))
            self.assertIn("</html>", html)

    def test_document_is_self_contained_no_external_refs(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            html = self.render(d)
            # No network dependency: no external scripts/stylesheets/images.
            self.assertNotIn("src=\"http", html)
            self.assertNotIn("href=\"http", html)
            self.assertNotIn("<link", html)


class TestDataIsland(HtmlTestBase):
    def test_every_issue_appears_in_the_island(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            self.seed(d, "Beta")
            data = island(self.render(d))
            titles = {i["title"] for i in data["issues"]}
            self.assertEqual(titles, {"Alpha", "Beta"})

    def test_issue_carries_the_expected_fields(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha", priority="high")
            data = island(self.render(d))
            issue = data["issues"][0]
            for key in ("id", "title", "status", "priority", "kind", "labels",
                        "parent", "children", "requires", "dependents", "body"):
                self.assertIn(key, issue)
            self.assertEqual(issue["priority"], "high")

    def test_config_vocabulary_is_embedded(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            data = island(self.render(d))
            self.assertIn("config", data)
            self.assertEqual([s["name"] for s in data["config"]["statuses"]],
                             ["backlog", "ongoing", "done"])
            self.assertIn("medium", data["config"]["priorities"])


class TestCommandCopy(HtmlTestBase):
    def test_config_carries_a_command_prefix(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            data = island(self.render(d))
            self.assertIn("cmd", data["config"])
            self.assertTrue(data["config"]["cmd"])

    def test_cmd_override_flows_into_the_island(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            ctx = self.h.build_ctx_from(str(d))
            data = island(self.h.render_html(ctx, cmd="./trck"))
            self.assertEqual(data["config"]["cmd"], "./trck")

    def test_default_cmd_is_trck_when_on_path(self):
        orig = self.h.shutil.which
        self.h.shutil.which = lambda name: "/usr/bin/trck"
        try:
            self.assertEqual(self.h._default_cmd(), "trck")
        finally:
            self.h.shutil.which = orig

    def test_default_cmd_falls_back_to_engine_path_when_off_path(self):
        orig = self.h.shutil.which
        self.h.shutil.which = lambda name: None
        try:
            self.assertTrue(self.h._default_cmd().endswith("trck"))
        finally:
            self.h.shutil.which = orig

    def test_document_includes_the_staging_ui(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            html = self.render(d)
            self.assertIn('id="pending"', html)
            self.assertIn("Copy", html)

    def test_cli_cmd_flag_overrides_prefix(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.h.main(["--dir", str(d), "-o", "-", "--cmd", "xyzzy"])
            data = island(buf.getvalue())
            self.assertEqual(data["config"]["cmd"], "xyzzy")


class TestBodyEscaping(HtmlTestBase):
    def test_body_cannot_break_out_of_the_script_island(self):
        payload = 'Danger: </script><script>alert(1)</script> & <b>bold</b> "q"'
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            self.body_file(d, iid).write_text(payload)
            html = self.render(d)
            # The raw closing tag from the body must NOT appear verbatim; if it did,
            # the island would be truncated. island() round-trips the exact payload.
            data = island(html)
            self.assertEqual(data["issues"][0]["body"], payload)


class TestEdges(HtmlTestBase):
    def test_parent_child_and_dependency_edges(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            pid = self.seed(d, "Epic")
            dep = self.seed(d, "Blocker")
            child = self.seed(d, "Child", parent=pid, depends=dep)
            data = island(self.render(d))
            by_id = {i["id"]: i for i in data["issues"]}
            self.assertIn(child, by_id[pid]["children"])
            self.assertEqual(by_id[pid]["parent"], None)
            self.assertIn(dep, by_id[child]["requires"])
            self.assertIn(child, by_id[dep]["dependents"])


class TestCli(HtmlTestBase):
    def test_default_output_is_issues_html_in_tracker_dir(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            self.h.main(["--dir", str(d)])
            out = Path(d) / "issues.html"
            self.assertTrue(out.is_file())
            self.assertIn("Alpha", out.read_text())

    def test_dash_o_writes_to_stdout(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.h.main(["--dir", str(d), "-o", "-"])
            self.assertIn("Alpha", buf.getvalue())
            self.assertFalse((Path(d) / "issues.html").exists())


if __name__ == "__main__":
    unittest.main()
