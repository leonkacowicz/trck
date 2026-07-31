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
import shutil
import subprocess
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

    def mv(self, d, iid, status):
        with redirect_stdout(io.StringIO()):
            self.t.cmd_mv(ns(dir=str(d), id=iid, status=status, resolution=None))

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

    def test_has_a_resizable_divider(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            html = self.render(d)
            self.assertIn('id="split"', html)

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

    def test_pr_is_exported_and_linked(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            url = "https://github.com/leonkacowicz/trck/pull/12"
            self.seed(d, "Alpha", pr=url)
            self.seed(d, "Beta")
            html = self.render(d)
            data = island(html)
            prs = {i["title"]: i["pr"] for i in data["issues"]}
            self.assertEqual(prs["Alpha"], url)
            self.assertIsNone(prs["Beta"])
            # the app builds the anchor from the exported value
            self.assertIn("prLink", html)

    def test_config_vocabulary_is_embedded(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            data = island(self.render(d))
            self.assertIn("config", data)
            self.assertEqual([s["name"] for s in data["config"]["statuses"]],
                             ["backlog", "ongoing", "in-review", "done"])
            self.assertIn("medium", data["config"]["priorities"])


@unittest.skipUnless(shutil.which("node"), "node not installed")
class TestGeneratedJsSyntax(HtmlTestBase):
    """The Python tests can't execute the embedded JS, so a stray Python-escape
    corrupting the generated script (e.g. an un-escaped `\\n` in a non-raw asset
    string) is invisible to them. `node --check` closes that gap when available."""
    def test_embedded_app_script_parses(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            html = self.render(d)
            scripts = re.findall(r'<script>\n(.*?)\n</script>', html, re.S)
            self.assertTrue(scripts, "no inline app <script> found")
            js = Path(tmp) / "app.js"
            js.write_text(scripts[-1])
            r = subprocess.run(["node", "--check", str(js)],
                               capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr)


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


class TestDependencyGraph(HtmlTestBase):
    def test_model_exposes_authored_dependency_edges(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            b = self.seed(d, "Blocker")
            a = self.seed(d, "Needs it", depends=b)
            data = island(self.render(d))
            self.assertIn("edges", data)
            self.assertIn({"from": b, "to": a}, data["edges"])

    def test_containment_is_not_a_dependency_edge(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            p = self.seed(d, "Parent")
            c = self.seed(d, "Child", parent=p)
            data = island(self.render(d))
            pairs = {(e["from"], e["to"]) for e in data["edges"]}
            self.assertNotIn((p, c), pairs)
            self.assertNotIn((c, p), pairs)

    def test_edges_empty_when_no_dependencies(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Lonely")
            data = island(self.render(d))
            self.assertEqual(data["edges"], [])

    def test_graph_view_ui_is_present(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            html = self.render(d)
            self.assertIn('id="graph"', html)
            self.assertIn('data-view="graph"', html)

    def test_graph_has_done_filter_controls(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            html = self.render(d)
            self.assertIn("include done chains", html)
            self.assertIn("omit done", html)


class TestTreeView(HtmlTestBase):
    def test_model_lists_roots(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            top = self.seed(d, "Top")
            parent = self.seed(d, "Parent")
            child = self.seed(d, "Child", parent=parent)
            data = island(self.render(d))
            self.assertIn("roots", data)
            self.assertIn(top, data["roots"])
            self.assertIn(parent, data["roots"])
            self.assertNotIn(child, data["roots"])
            self.assertEqual(data["roots"], sorted(data["roots"]))

    def test_parent_progress_rolls_up_and_leaves_are_null(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            parent = self.seed(d, "Parent")
            c1 = self.seed(d, "One", parent=parent)
            self.seed(d, "Two", parent=parent)
            self.mv(d, c1, "done")
            data = island(self.render(d))
            by_id = {i["id"]: i for i in data["issues"]}
            self.assertEqual(by_id[parent]["progress"]["pct"], 50)
            self.assertIsNone(by_id[c1]["progress"])

    def test_tree_view_ui_is_present(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            html = self.render(d)
            self.assertIn('id="tree"', html)
            self.assertIn('data-view="tree"', html)


class TestMarkdownBodies(HtmlTestBase):
    def test_body_render_toggle_and_renderer_present(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            html = self.render(d)
            self.assertIn("renderMarkdown", html)   # the subset renderer
            self.assertIn("bodytog", html)          # the raw/rendered toggle

    def test_raw_body_still_shipped_and_escaped(self):
        payload = '## Head\n</script><script>alert(1)</script> **bold**'
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "A")
            self.body_file(d, iid).write_text(payload)
            html = self.render(d)
            data = island(html)
            # The raw markdown source rides the island intact...
            self.assertEqual(data["issues"][0]["body"], payload)
            # ...and the dangerous tag never appears un-neutralised in the document.
            self.assertNotIn("<script>alert(1)</script>", html)


class TestBoardView(HtmlTestBase):
    def test_board_view_ui_is_present(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            html = self.render(d)
            self.assertIn('id="board"', html)
            self.assertIn('data-view="board"', html)


class TestReadyView(HtmlTestBase):
    """The ready view mirrors `trck ready`: actionable leaves ranked by the demand
    cone. The ranking itself is authored once in the engine and shipped as data —
    these tests pin the exported values, which is what the client sorts on."""

    def test_demand_vector_counts_the_cone_by_priority(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker", priority="low")
            self.seed(d, "Urgent dependent", priority="urgent", depends=blocker)
            by_id = {i["id"]: i for i in island(self.render(d))["issues"]}
            # priorities are urgent, high, medium, low, lowest (+ a trailing bucket
            # for unconfigured ones): the cone is the low blocker plus its urgent
            # dependent, so slot 0 and slot 3 each hold one.
            self.assertEqual(by_id[blocker]["demand"], [1, 0, 0, 1, 0, 0])

    def test_a_lone_issue_is_its_own_cone(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Lonely", priority="high")
            by_id = {i["id"]: i for i in island(self.render(d))["issues"]}
            self.assertEqual(by_id[iid]["demand"], [0, 1, 0, 0, 0, 0])

    def test_demand_source_names_the_issue_that_lifts_a_row(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker", priority="low")
            urgent = self.seed(d, "Urgent dependent", priority="urgent", depends=blocker)
            by_id = {i["id"]: i for i in island(self.render(d))["issues"]}
            self.assertEqual(by_id[blocker]["demand_source"], urgent)

    def test_demand_source_is_null_when_the_row_already_leads_its_cone(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker", priority="urgent")
            self.seed(d, "Low dependent", priority="low", depends=blocker)
            by_id = {i["id"]: i for i in island(self.render(d))["issues"]}
            self.assertIsNone(by_id[blocker]["demand_source"])

    def test_a_low_blocker_outranks_a_high_issue_blocking_nothing(self):
        """The whole point of the ranking, expressed in the shipped vectors: the
        client compares them slot by slot, highest priority first."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker", priority="low")
            self.seed(d, "Urgent dependent", priority="urgent", depends=blocker)
            lone = self.seed(d, "Lone high", priority="high")
            by_id = {i["id"]: i for i in island(self.render(d))["issues"]}
            self.assertGreater(by_id[blocker]["demand"], by_id[lone]["demand"])

    def test_ready_view_ui_is_present(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            html = self.render(d)
            self.assertIn('id="ready"', html)
            self.assertIn('data-view="ready"', html)

    def test_hidden_panes_stay_hidden_despite_their_own_display(self):
        """`setView` hides the inactive panes with the `hidden` attribute, but a pane
        that sets `display` in our own stylesheet (`.board { display: flex }`) outranks
        the UA's `[hidden] { display: none }` — author styles beat user-agent styles at
        any specificity. Without an author-level override the board keeps painting over
        every pane declared after it, which is every pane added from here on."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            css = self.render(d)
            self.assertRegex(css, r"\[hidden\]\s*\{[^}]*display:\s*none\s*!important")

    def test_ready_flag_marks_actionable_leaves_only(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker")
            waiting = self.seed(d, "Waiting", depends=blocker)
            by_id = {i["id"]: i for i in island(self.render(d))["issues"]}
            self.assertTrue(by_id[blocker]["ready"])
            self.assertFalse(by_id[waiting]["ready"])


class TestFilterCheckboxes(HtmlTestBase):
    """Status and priority are multi-select checkbox facets, not single-choice
    dropdowns, and not every view applies them."""

    def test_facet_containers_replace_the_dropdowns(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            html = self.render(d)
            self.assertIn('id="ffstatus"', html)
            self.assertIn('id="ffpriority"', html)
            self.assertNotIn('<select id="fstatus">', html)
            self.assertNotIn('<select id="fpriority">', html)

    def test_facet_boxes_reuse_the_shared_checkbox_helper(self):
        """`checkbox()` already backs the graph's toggles; the facets are the same
        control, so a bare `assertIn('checkbox')` would pass without them."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            m = re.search(r"function fillFacet\(.*?\n\}", self.render(d), re.S)
            self.assertIsNotNone(m, "no fillFacet builder found")
            self.assertIn("checkbox(", m.group(0))


@unittest.skipUnless(shutil.which("node"), "node not installed")
class TestFilterFacetSemantics(HtmlTestBase):
    """Which facets a view applies, and what an empty selection means, are decided by
    a pure core the page brackets with sentinel comments. These tests slice that core
    out of the rendered document and run it — the shipped code, not a copy of it."""

    CORE = re.compile(r"// --- filter facets \(pure.*?\n(.*?)// --- end filter facets",
                      re.S)

    def probe(self, tmp, expr):
        d = make_tracker(tmp, {})
        self.seed(d, "A")
        js = re.findall(r'<script>\n(.*?)\n</script>', self.render(d), re.S)[-1]
        m = self.CORE.search(js)
        self.assertIsNotNone(m, "filter-facet sentinel comments not found")
        drv = Path(tmp) / "drv.mjs"
        drv.write_text(m.group(1) + "\nconsole.log(JSON.stringify(" + expr + "));\n")
        r = subprocess.run(["node", str(drv)], capture_output=True, text=True)
        self.assertEqual(r.returncode, 0, r.stderr)
        return json.loads(r.stdout)

    def test_list_and_tree_apply_both_facets(self):
        with TemporaryDirectory() as tmp:
            self.assertEqual(self.probe(tmp, "[facetsFor('list'), facetsFor('tree')]"),
                             [["status", "priority"], ["status", "priority"]])

    def test_graph_applies_both_facets(self):
        with TemporaryDirectory() as tmp:
            self.assertEqual(self.probe(tmp, "facetsFor('graph')"), ["status", "priority"])

    def test_board_applies_priority_but_not_status(self):
        with TemporaryDirectory() as tmp:
            self.assertEqual(self.probe(tmp, "facetsFor('board')"), ["priority"])

    def test_ready_applies_neither_facet(self):
        with TemporaryDirectory() as tmp:
            self.assertEqual(self.probe(tmp, "facetsFor('ready')"), [])

    def test_an_empty_selection_means_no_filter(self):
        with TemporaryDirectory() as tmp:
            self.assertTrue(
                self.probe(tmp, "passesFacet('list', 'status', new Set(), 'done')"))

    def test_a_selection_admits_only_what_is_checked(self):
        with TemporaryDirectory() as tmp:
            self.assertEqual(
                self.probe(tmp, "[passesFacet('list', 'status', new Set(['todo']), 'todo'),"
                                " passesFacet('list', 'status', new Set(['todo']), 'done')]"),
                [True, False])

    def test_a_view_ignores_a_facet_it_does_not_apply(self):
        """The selection survives a trip through board or ready — it is simply not
        consulted there — so returning to list restores what was checked."""
        with TemporaryDirectory() as tmp:
            self.assertEqual(
                self.probe(tmp, "[passesFacet('board', 'status', new Set(['todo']), 'done'),"
                                " passesFacet('ready', 'priority', new Set(['high']), 'low')]"),
                [True, True])


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


class TestIdPrefixHighlight(HtmlTestBase):
    def test_model_carries_shortest_unique_prefix_len(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            for t in ("Alpha", "Beta", "Gamma", "Delta", "Epsilon"):
                self.seed(d, t)
            data = island(self.render(d))
            ids = [i["id"] for i in data["issues"]]
            expect = self.t.unique_prefix_lens(ids)  # the same helper the CLI uses
            for i in data["issues"]:
                self.assertEqual(i["plen"], expect[i["id"]])
                self.assertGreaterEqual(i["plen"], 1)
                self.assertLessEqual(i["plen"], len(i["id"]))

    def test_document_styles_and_marks_up_the_prefix(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            html = self.render(d)
            # The emphasised-prefix class is styled and produced by the id renderer.
            self.assertIn(".idpre", html)
            self.assertIn("idpre", html)
            self.assertIn("plen", html)


if __name__ == "__main__":
    unittest.main()
