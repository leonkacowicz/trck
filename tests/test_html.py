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


def js_pieces(js, *names):
    """Lift top-level `const` lines and `function` blocks out of the app script by name.

    A name is looked for as a single-line `const` declaration first — which covers both
    plain constants and one-line arrow helpers — and then as a `function` block. Every
    top-level function in the emitted script closes with a `}` in column 0, so the block
    boundary is unambiguous without parsing JavaScript."""
    out = []
    for n in names:
        for pat in (rf"^const {re.escape(n)}\b.*$", rf"^function {re.escape(n)}\([\s\S]*?^\}}"):
            m = re.search(pat, js, re.M)
            if m:
                out.append(m.group(0))
                break
        else:
            raise AssertionError(f"could not lift `{n}` out of the app script")
    return "\n".join(out)


class HtmlTestBase(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.h = load_tool()

    def seed(self, d, title="Item", **over):
        a = dict(dir=str(d), title=title, priority="medium", parent=None,
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

    def test_font_stacks_are_declared_once(self):
        """Seven literal copies of a stack is how they drift apart. Both live in a custom
        property, and a sentinel family from each must therefore appear exactly once."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            html = self.render(d)
            self.assertRegex(html, r"--sans:")
            self.assertRegex(html, r"--mono:")
            self.assertEqual(html.count("SFMono-Regular"), 1)
            self.assertEqual(html.count("Liberation Sans"), 1)

    def test_preferred_families_lead_the_fallbacks(self):
        """A stack is the whole mechanism: name the good fonts first and the browser takes
        the first one installed, resolving locally with no network. The mono list stays
        conservative on purpose — graph labels are truncated by character count against a
        fixed box, so a family with a wider advance would overflow it."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            html = self.render(d)
            sans = re.search(r"--sans:([^;]*);", html).group(1)
            mono = re.search(r"--mono:([^;]*);", html).group(1)
            self.assertLess(sans.index("Inter"), sans.index("system-ui"))
            self.assertLess(sans.index("system-ui"), sans.index("sans-serif"))
            # Nothing ahead of the system mono, and every name in it sits at the same
            # ~0.6em advance the label budget assumes.
            self.assertTrue(mono.strip().startswith("ui-monospace"), mono)
            self.assertIn("monospace", mono)

    def test_counts_and_percentages_use_tabular_figures(self):
        """Digits that change width make a re-render look like a layout shift."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            self.assertIn("tabular-nums", self.render(d))

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
            self.seed(d, "Alpha", review_url=url)
            self.seed(d, "Beta")
            html = self.render(d)
            data = island(html)
            prs = {i["title"]: i["review_url"] for i in data["issues"]}
            self.assertEqual(prs["Alpha"], url)
            self.assertIsNone(prs["Beta"])
            # the app builds the anchor from the exported value
            self.assertIn("reviewLink", html)

    def test_config_vocabulary_is_embedded(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            data = island(self.render(d))
            self.assertIn("config", data)
            self.assertEqual([s["name"] for s in data["config"]["statuses"]],
                             ["backlog", "ongoing", "in-review", "done"])
            self.assertIn("medium", data["config"]["priorities"])

    def test_repo_is_the_tracker_parent_dir_name(self):
        # The page header names the *project*, so it comes from the directory that
        # holds the tracker — never from `update.repo`, which is the engine's release
        # channel (and is always populated from DEFAULT_CONFIG, so it would otherwise
        # title every consumer's page with trck's own upstream slug).
        with TemporaryDirectory() as tmp:
            proj = Path(tmp) / "prjname"
            proj.mkdir()
            d = make_tracker(proj, {"update": {"repo": "someone/upstream"}})
            self.seed(d, "Alpha")
            data = island(self.render(d))
            self.assertEqual(data["repo"], "prjname")


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

    def test_hovering_a_node_accents_its_edges(self):
        """Hover wiring is DOM-bound, so what is checkable here is that every piece exists:
        the handlers on the node group, a rule that accents a lifted edge, and the accented
        arrowhead it switches to. A `.gedge.hi` the stylesheet never mentions would leave a
        hover that changes nothing, which is the regression worth catching."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            html = self.render(d)
            self.assertIn("onmouseenter:", html)
            self.assertIn("onmouseleave:", html)
            self.assertRegex(html, r"\.gedge\.hi\s*\{")
            # A shared marker cannot take the accent from the path, so the highlighted
            # state swaps in its own head rather than recolouring the one it has. Both
            # heads must be defined, and the rule must point at the accented one.
            self.assertRegex(html, r"head\('arrow',\s*'var\(--muted\)'\)")
            self.assertRegex(html, r"head\('arrow-hi',\s*'var\(--accent\)'\)")
            self.assertRegex(html, r"\.gedge\.hi\s*\{[^}]*url\(#arrow-hi\)")

    def test_edges_are_grouped_so_a_lifted_one_stays_under_the_nodes(self):
        """Raising a highlighted edge above its neighbours means re-appending it, and SVG
        paints in document order — so if edges were siblings of the nodes it would climb
        over the boxes too. They go in their own group, which bounds how far it can rise."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            self.assertIn("edgeLayer", self.render(d))

    def test_graph_has_done_filter_controls(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            html = self.render(d)
            self.assertIn("include done chains", html)
            self.assertIn("omit done", html)

    def test_edges_stop_at_the_arrowhead_base(self):
        """The head is anchored by its base (refX 0) at a curve that ends ARROW short of
        the node, so the stroke never runs under the triangle and out past its tip."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            html = self.render(d)
            self.assertIn("const ARROW = 10;", html)
            self.assertIn("refX: '0'", html)
            # Sized in user units, so the head length is independent of stroke-width and
            # matches the gap the curve leaves for it exactly.
            self.assertIn("markerUnits: 'userSpaceOnUse'", html)
            # The trim lives on the shared endpoint now that an edge can arrive there
            # after several routed hops, rather than being applied at the one curve.
            self.assertIn("y2 = b.y - ARROW", html)


class TestGraphLayout(HtmlTestBase):
    """Drive `layoutComponent` for real, under node.

    The app script binds itself to the DOM as it loads, so it cannot be evaluated whole
    here — but the layout helpers are pure, so they are lifted out and run alone. That
    buys actual coordinates to assert on instead of string matches against the source."""

    # Recover the left-to-right rows the layout settled on from the coordinates it returned.
    # The crossing count comes back from `layoutComponent` itself rather than being recomputed
    # here: scoring needs the split graph, which only the layout builds.
    HARNESS = """
const out = layoutComponent(%s, %s);
const ys = [...new Set(Object.values(out.local).map(p => p.y))].sort((a, b) => a - b);
const rows = ys.map(y => Object.keys(out.local).filter(id => out.local[id].y === y)
                                               .sort((a, b) => out.local[a].x - out.local[b].x));
console.log(JSON.stringify({ out, rows, xings: out.xings,
                             NODE_W, NODE_H, COL_GAP, ROW_GAP, REFINE_MAX }));
"""

    def layout(self, comp, preds):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            js = re.findall(r'<script>\n(.*?)\n</script>', self.render(d), re.S)[-1]
            src = js_pieces(js, "NODE_W", "REFINE_MAX", "isBend", "layerOf", "isotonic",
                            "crossings", "refine", "orderRows", "splitLongEdges",
                            "layoutComponent")
            f = Path(tmp) / "layout.js"
            f.write_text(src + self.HARNESS % (json.dumps(comp), json.dumps(preds)))
            r = subprocess.run(["node", str(f)], capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr)
            return json.loads(r.stdout)

    def bend(self, out, frm, to):
        return out["bends"].get(f"{frm} {to}", [])

    def test_a_lone_child_sits_under_its_only_parent(self):
        # Two roots share the top row; only the right-hand one has a child. On the plain
        # grid that child took column 0 and its edge raked across to the left.
        r = self.layout(["a", "b", "c"], {"c": ["b"]})
        x = r["out"]["local"]
        self.assertEqual(x["c"]["x"], x["b"]["x"])

    def test_a_parent_centres_over_its_children(self):
        r = self.layout(["a", "b", "c"], {"b": ["a"], "c": ["a"]})
        x = r["out"]["local"]
        self.assertAlmostEqual(x["a"]["x"], (x["b"]["x"] + x["c"]["x"]) / 2)

    def test_two_nodes_wanting_the_same_place_stay_ordered_and_apart(self):
        # Both children hang off the one parent, so both barycentres land on it. Placement
        # cannot honour that, and what it must not do is stack them or let them swap.
        r = self.layout(["a", "p", "q"], {"p": ["a"], "q": ["a"]})
        x, step = r["out"]["local"], r["NODE_W"] + r["COL_GAP"]
        self.assertLess(x["p"]["x"], x["q"]["x"])
        self.assertGreaterEqual(x["q"]["x"] - x["p"]["x"], step)

    def test_a_row_is_reordered_when_that_removes_a_crossing(self):
        # Alphabetically row 1 is [s, t] while their blockers are [p, q] — s hangs off q
        # and t off p, so the two edges cross. Swapping the row undoes it.
        r = self.layout(["p", "q", "s", "t"], {"s": ["q"], "t": ["p"]})
        x = r["out"]["local"]
        self.assertLess(x["t"]["x"], x["s"]["x"])
        self.assertEqual(x["t"]["x"], x["p"]["x"])
        self.assertEqual(x["s"]["x"], x["q"]["x"])

    def test_a_crossing_free_component_keeps_its_alphabetical_order(self):
        # Same shape, untangled: the sweep has nothing to win, so the row is left alone
        # and can still be read by name from left to right.
        r = self.layout(["p", "q", "s", "t"], {"s": ["p"], "t": ["q"]})
        x = r["out"]["local"]
        self.assertLess(x["p"]["x"], x["q"]["x"])
        self.assertLess(x["s"]["x"], x["t"]["x"])

    # A 3x3 found by exhaustive search: the sweeps settle at two crossings and cannot see
    # the single relocation that reaches the optimum of one. The sweeps move a whole row
    # at a time, so no reordering they can express improves on where they stopped.
    STALLED = (["n0", "n1", "n2", "n3", "n4", "n5"],
               {"n3": ["n0", "n1", "n2"], "n4": ["n0", "n2"], "n5": ["n0"]})

    def test_relocating_one_node_finishes_what_the_sweeps_could_not(self):
        self.assertEqual(self.layout(*self.STALLED)["xings"], 1)

    def test_a_component_past_the_refine_cap_keeps_the_sweeps_result(self):
        """Refinement is quadratic in the row and recounts crossings per candidate, so it
        is skipped on big components and the sweep's order stands. Same stalled shape,
        padded over the cap by a chain that adds rows without adding crossings."""
        comp, preds = self.STALLED
        comp, preds = list(comp), dict(preds)
        prev = "n5"
        for i in range(70):
            nxt = f"p{i:02d}"
            comp.append(nxt)
            preds[nxt] = [prev]
            prev = nxt
        r = self.layout(comp, preds)
        self.assertGreater(len(comp), r["REFINE_MAX"])
        self.assertEqual(r["xings"], 2)

    def test_an_edge_skipping_a_layer_bends_through_the_row_it_crosses(self):
        # a -> b -> z puts z two layers below a, so a's own edge to z flies over row 1.
        # It gets a placeholder there, which is the point on the row the edge routes through.
        r = self.layout(["a", "b", "z"], {"b": ["a"], "z": ["a", "b"]})
        out = r["out"]
        self.assertEqual(len(self.bend(out, "a", "z")), 1)
        self.assertEqual(self.bend(out, "a", "z")[0]["y"], r["NODE_H"] + r["ROW_GAP"])
        # A unit-length edge has no row to cross and so no placeholder.
        self.assertEqual(self.bend(out, "b", "z"), [])

    def test_placeholders_do_not_leak_into_the_drawn_nodes(self):
        comp = ["a", "b", "z"]
        r = self.layout(comp, {"b": ["a"], "z": ["a", "b"]})
        self.assertCountEqual(r["out"]["local"].keys(), comp)

    def test_a_long_edge_is_ordered_around_rather_than_across(self):
        """Without a placeholder the long edge holds no slot in the row it crosses, so
        nothing scores it and nothing can move it out of the way. With one, the crossing
        count sees it and the ordering puts it on the side that keeps the row clean."""
        # a and b lead; p sits under b; z waits on both a and p. a's edge to z crosses row 1,
        # where p already lives — and it must pass to p's left, since a is left of b.
        r = self.layout(["a", "b", "p", "z"], {"p": ["b"], "z": ["a", "p"]})
        out = r["out"]
        self.assertEqual(r["xings"], 0)
        self.assertLess(self.bend(out, "a", "z")[0]["x"], out["local"]["p"]["x"])

    def test_the_median_barycentre_reaches_an_order_the_mean_misses(self):
        """Eight nodes, found by search, that a mean barycentre settles on at two crossings
        and a median clears entirely. An outlying neighbour drags the mean, so a row can be
        ordered by a position no neighbour actually occupies; the median only ever names one
        of them. It also carries a proven bound (Eades-Wormald) where the mean does not.

        Long edges are what make this bite: the six rows here come from splitting, and every
        placeholder is one more neighbour with a say in where its row sits."""
        r = self.layout(["n0", "n1", "n2", "n3", "n4", "n5", "n6", "n7"],
                        {"n2": ["n1"], "n3": ["n1"], "n4": ["n3"],
                         "n5": ["n0", "n1", "n3", "n4"],
                         "n6": ["n1", "n2", "n3", "n4", "n5"], "n7": ["n5", "n6"]})
        self.assertEqual(r["xings"], 0)

    def test_layers_stack_by_row_gap_and_the_box_covers_the_nodes(self):
        r = self.layout(["a", "b", "c"], {"b": ["a"], "c": ["a"]})
        out, x = r["out"], r["out"]["local"]
        self.assertEqual(x["a"]["y"], 0)
        self.assertEqual(x["b"]["y"], r["NODE_H"] + r["ROW_GAP"])
        # Shifting nodes off the grid means w can no longer be counted in columns.
        self.assertEqual(min(v["x"] for v in x.values()), 0)
        self.assertEqual(out["w"], max(v["x"] for v in x.values()) + r["NODE_W"])


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

    def test_parents_start_collapsed(self):
        """The tree opens on its roots, not on every leaf in the tracker — the point of a
        hierarchy view is to choose what to expand."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            self.assertRegex(self.render(d),
                             r"collapsed:\s*new Set\(DATA\.issues\.filter\(")

    def test_only_a_search_overrides_a_collapsed_parent(self):
        """A text query is a hunt for something that may be buried, so it expands past a
        collapsed parent — a match hidden inside one looks like no match. Unchecking a
        facet is not that: it narrows the population on show, and blowing the whole tree
        open because `done` was hidden loses the shape the user arranged.

        The empty-state wording still keys off the wider `filterActive`, since a facet that
        leaves nothing does owe the reader "no matching issues" rather than "no issues"."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            html = self.render(d)
            self.assertIn("const searching = !!state.q;", html)
            self.assertRegex(html, r"collapsed\s*=\s*!searching && state\.collapsed\.has\(id\)")
            self.assertRegex(html, r"text:\s*filterActive\(\)\s*\?\s*'No matching issues\.'")

    def test_tree_rows_mark_ready_leaves(self):
        """Actionable work should be visible in context, not only in the ready view —
        the tree is where you go to ask "what's left under this epic?"."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            m = re.search(r"function renderTreeNode\(.*?\n\}", self.render(d), re.S)
            self.assertIsNotNone(m, "no renderTreeNode builder found")
            self.assertIn("i.ready", m.group(0))
            self.assertIn("badge ready", m.group(0))

    def test_the_ready_badge_is_styled(self):
        """A class the stylesheet never mentions renders as a plain badge, which is
        exactly what this one must not look like."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            self.assertRegex(self.render(d), r"\.badge\.ready\s*\{")


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

    def test_an_empty_selection_admits_nothing(self):
        """The boxes start checked, so they say what is showing. That only holds if they
        mean it all the way down: unchecking the last one shows nothing, rather than
        wrapping around to showing everything again."""
        with TemporaryDirectory() as tmp:
            self.assertFalse(
                self.probe(tmp, f"passesFacet('list', 'status', new Set(), 'done', {self.VOCAB})"))

    def test_a_value_the_facet_never_offers_is_not_filtered_out(self):
        """An issue can carry a status the config no longer lists. There is no box for it,
        so it can never be checked — and must not be hidden by one nobody can see."""
        with TemporaryDirectory() as tmp:
            self.assertTrue(self.probe(
                tmp, f"passesFacet('list', 'status', new Set(['todo']), 'retired', {self.VOCAB})"))

    VOCAB = "new Set(['todo', 'done'])"

    def test_a_selection_admits_only_what_is_checked(self):
        with TemporaryDirectory() as tmp:
            self.assertEqual(
                self.probe(tmp,
                           f"[passesFacet('list', 'status', new Set(['todo']), 'todo', {self.VOCAB}),"
                           f" passesFacet('list', 'status', new Set(['todo']), 'done', {self.VOCAB})]"),
                [True, False])

    def test_a_view_ignores_a_facet_it_does_not_apply(self):
        """The selection survives a trip through board or ready — it is simply not
        consulted there — so returning to list restores what was checked."""
        with TemporaryDirectory() as tmp:
            self.assertEqual(
                self.probe(tmp,
                           f"[passesFacet('board', 'status', new Set(['todo']), 'done', {self.VOCAB}),"
                           f" passesFacet('ready', 'priority', new Set(['high']), 'low', {self.VOCAB})]"),
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
