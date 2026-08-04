import io
import re
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory
from unittest import mock

from tests.helpers import load_trck, make_tracker, ns


class TestRead(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title="Item", parent=None, priority="high",
             points=None, depends=None):
        a = ns(dir=str(d), title=title, priority=priority, parent=parent,
               points=points, depends=depends, spec=None, slug=None)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(a)
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def listing(self, d, **over):
        """cmd_list defaulted to the flat view (the stable regression baseline);
        override `flat=False` for nested-forest tests."""
        a = dict(dir=str(d), status=None, priority=None, label=None,
                 parent=None, match=None, sort=None, blocked=False, orphan=False,
                 flat=True, id=None)
        a.update(over)
        return self.cap(self.t.cmd_list, ns(**a))

    def ready(self, d, **over):
        """cmd_ready with flags defaulted; override per test."""
        a = dict(dir=str(d), next=False)
        a.update(over)
        return self.cap(self.t.cmd_ready, ns(**a))

    def cap(self, fn, args):
        buf = io.StringIO()
        with redirect_stdout(buf):
            fn(args)
        return buf.getvalue()

    def test_show_human_metadata_and_body(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Hello")
            out = self.cap(self.t.cmd_show, ns(dir=str(d), id=id1, json=False))
            self.assertIn("title", out)        # aligned key: value, not raw JSON
            self.assertIn("Hello", out)
            self.assertNotIn('"id": "1"', out)
            self.assertIn("--- body ---", out)
            self.assertIn("# Hello", out)

    def test_show_json_flag(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Hello")
            out = self.cap(self.t.cmd_show, ns(dir=str(d), id=id1, json=True))
            self.assertIn(f'"id": "{id1}"', out)
            self.assertIn("--- body ---", out)

    def test_list_filters_by_status(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "A")
            id2 = self.seed(d, "B")
            self.t.cmd_mv(ns(dir=str(d), id=id2, status="ongoing", resolution=None))
            out = self.cap(self.t.cmd_list, ns(dir=str(d), status="ongoing",
                                               priority=None, parent=None,
                                               flat=True, id=None))
            self.assertIn(f"#{id2}", out)
            self.assertNotIn(f"#{id1}", out)

    def test_list_filters_by_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Child", parent=id1)
            id3 = self.seed(d, "Loose")
            out = self.cap(self.t.cmd_list, ns(dir=str(d), status=None,
                                               priority=None, parent=id1,
                                               flat=True, id=None))
            self.assertIn(f"#{id2}", out)
            self.assertNotIn(f"#{id1}", out)
            self.assertNotIn(f"#{id3}", out)

    def test_list_status_multi_and_negated(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "A")
            id2 = self.seed(d, "B")
            id3 = self.seed(d, "C")
            self.t.cmd_mv(ns(dir=str(d), id=id2, status="ongoing", resolution=None))
            self.t.cmd_mv(ns(dir=str(d), id=id3, status="done", resolution=None))
            out = self.listing(d, status="backlog,ongoing")
            self.assertIn(f"#{id1}", out)
            self.assertIn(f"#{id2}", out)
            self.assertNotIn(f"#{id3}", out)
            out = self.listing(d, status="!done")
            self.assertIn(f"#{id1}", out)
            self.assertIn(f"#{id2}", out)
            self.assertNotIn(f"#{id3}", out)

    def test_list_match_is_case_insensitive_substring(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Fix the parser")
            id2 = self.seed(d, "Add a feature")
            out = self.listing(d, match="PARSER")
            self.assertIn(f"#{id1}", out)
            self.assertNotIn(f"#{id2}", out)

    def test_list_sort_by_priority(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Low one", priority="low")
            id2 = self.seed(d, "High one", priority="high")
            id3 = self.seed(d, "Mid one", priority="medium")
            out = self.listing(d, sort="priority")
            self.assertLess(out.index(f"#{id2}"), out.index(f"#{id3}"))
            self.assertLess(out.index(f"#{id3}"), out.index(f"#{id1}"))

    def test_list_sort_by_points(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Small", points=1)
            id2 = self.seed(d, "Big", points=8)
            id3 = self.seed(d, "Mid", points=3)
            out = self.listing(d, sort="points")
            self.assertLess(out.index(f"#{id2}"), out.index(f"#{id3}"))
            self.assertLess(out.index(f"#{id3}"), out.index(f"#{id1}"))

    def test_list_blocked_only(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Dep")
            id2 = self.seed(d, "Blocked", depends=id1)
            id3 = self.seed(d, "Free")
            out = self.listing(d, blocked=True)
            self.assertIn(f"#{id2}", out)
            # only #id2 is listed as a row; #id1 appears only inside its `needs` note
            self.assertEqual("", self.row_for(out, id1))
            self.assertEqual("", self.row_for(out, id3))
            # once the dependency is terminal, nothing is blocked
            self.t.cmd_mv(ns(dir=str(d), id=id1, status="done", resolution=None))
            self.assertEqual(self.listing(d, blocked=True), "")

    @staticmethod
    def row_for(out, issue_id):
        """The output line whose OWN id (the first #NNN on the line, the id column) is
        `issue_id` — not a line that merely mentions it in a needs/blocks annotation."""
        tok = f"#{issue_id}"
        for ln in out.splitlines():
            m = re.search(r"#[A-Za-z0-9]+", ln)
            if m and m.group(0) == tok:
                return ln
        return ""

    def test_list_annotates_blocked_row_with_needs(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Dep")
            id2 = self.seed(d, "Blocked", depends=id1)
            out = self.listing(d)
            self.assertIn(f"needs #{id1}", self.row_for(out, id2))

    def test_list_needs_omits_terminal_blocker(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Dep")
            id2 = self.seed(d, "Blocked", depends=id1)
            self.t.cmd_mv(ns(dir=str(d), id=id1, status="done", resolution=None))
            out = self.listing(d)
            # block cleared: the dependent no longer advertises a blocker
            self.assertNotIn("needs", self.row_for(out, id2))

    def test_list_needs_lists_only_open_blockers(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "DepA")
            id2 = self.seed(d, "DepB")
            id3 = self.seed(d, "Blocked", depends=f"{id1},{id2}")
            self.t.cmd_mv(ns(dir=str(d), id=id1, status="done", resolution=None))
            line = self.row_for(self.listing(d), id3)
            self.assertIn(f"needs #{id2}", line)               # still open
            self.assertNotIn(f"#{id1}", line)                  # done -> dropped

    def test_list_annotates_blocker_row_with_blocks(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Dep")
            id2 = self.seed(d, "Blocked", depends=id1)
            line = self.row_for(self.listing(d), id1)
            self.assertIn(f"blocks #{id2}", line)

    def test_list_blocks_cleared_when_blocker_done(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Dep")
            id2 = self.seed(d, "Blocked", depends=id1)
            self.t.cmd_mv(ns(dir=str(d), id=id1, status="done", resolution=None))
            out = self.listing(d)
            # a done task blocks nothing — both sides of the edge go quiet
            self.assertNotIn("blocks", self.row_for(out, id1))
            self.assertNotIn("needs", self.row_for(out, id2))

    def test_list_block_annotations_are_plain_without_color(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Dep")
            id2 = self.seed(d, "Blocked", depends=id1)
            out = self.listing(d)
            self.assertIn(f"needs #{id1}", out)
            self.assertNotIn("\x1b[", out)                 # no ANSI when color is off

    def test_ready_has_no_block_annotations(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Dep")
            id2 = self.seed(d, "Blocked", depends=id1)
            out = self.ready(d)
            self.assertIn(f"#{id1}", out)
            self.assertNotIn("blocks", out)                # ready stays terse
            self.assertNotIn("needs", out)

    # --- inherited (ancestor-authored) dependencies in the `needs` note ---------
    # A parent's dependency is inherited by its whole subtree, so a child can be
    # blocked by an edge authored above it. The annotation spells that edge out
    # only when the authoring ancestor is NOT itself on screen — where it is, its
    # own row already carries the note and repeating it on every child is noise.

    def inherit_fixture(self, d):
        """`kid` under `par`, where `par` (not `kid`) authored the dep on `dep`."""
        dep = self.seed(d, "Dep")
        par = self.seed(d, "Parent epic", depends=dep)
        kid = self.seed(d, "Child leaf", parent=par)
        return dep, par, kid

    def test_list_spells_out_inherited_dep_when_ancestor_not_shown(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            dep, par, kid = self.inherit_fixture(d)
            # only the child matches, so its parent's row is absent from the output
            line = self.row_for(self.listing(d, match="Child leaf"), kid)
            self.assertIn(f"needs #{dep}", line)      # agrees with ready/is_blocked
            self.assertIn(f"(via #{par})", line)      # ...but the edge lives on par

    def test_list_omits_inherited_dep_when_ancestor_is_shown(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            dep, par, kid = self.inherit_fixture(d)
            out = self.listing(d)                     # unfiltered: par is printed too
            self.assertIn(f"needs #{dep}", self.row_for(out, par))
            self.assertNotIn("needs", self.row_for(out, kid))

    def test_list_nested_forest_keeps_ancestor_so_child_stays_quiet(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            dep, par, kid = self.inherit_fixture(d)
            # the forest keeps a match's ancestor spine as context -> par is on screen
            out = self.listing(d, flat=False, match="Child leaf")
            self.assertIn(f"#{par}", out)
            self.assertNotIn("needs", self.row_for(out, kid))

    def test_list_rooted_at_child_spells_out_inherited_dep(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            dep, par, kid = self.inherit_fixture(d)
            # rooting at the child cuts the spine above it: nothing else carries it
            line = self.row_for(self.listing(d, flat=False, id=kid), kid)
            self.assertIn(f"needs #{dep} (via #{par})", line)

    def test_list_inherited_dep_follows_its_own_authored_deps(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            dep, par, kid = self.inherit_fixture(d)
            own = self.seed(d, "Own dep")
            self.t.cmd_dep(ns(dir=str(d), id=kid, add=own, remove=None))
            line = self.row_for(self.listing(d, match="Child leaf"), kid)
            self.assertIn(f"needs #{own} #{dep} (via #{par})", line)

    def test_list_inherited_dep_clears_when_the_blocker_is_done(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            dep, par, kid = self.inherit_fixture(d)
            self.t.cmd_mv(ns(dir=str(d), id=dep, status="done", resolution=None))
            out = self.listing(d, match="Child leaf")
            self.assertNotIn("needs", self.row_for(out, kid))

    def test_list_inherited_dep_is_reported_once_down_the_spine(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            dep = self.seed(d, "Dep")
            top = self.seed(d, "Top epic", depends=dep)
            mid = self.seed(d, "Child mid", parent=top)
            kid = self.seed(d, "Child leaf", parent=mid)
            out = self.listing(d, match="Child")      # prints mid + kid, not top
            self.assertIn(f"needs #{dep} (via #{top})", self.row_for(out, mid))
            self.assertNotIn("needs", self.row_for(out, kid))  # mid already says it

    def test_list_blocks_stays_at_the_authored_altitude(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            dep, par, kid = self.inherit_fixture(d)
            line = self.row_for(self.listing(d), dep)
            self.assertIn(f"blocks #{par}", line)     # the subtree under par is implied
            self.assertNotIn(f"#{kid}", line)

    def test_list_orphan_only(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Child", parent=id1)
            out = self.listing(d, orphan=True)
            self.assertIn(f"#{id1}", out)
            self.assertNotIn(f"#{id2}", out)

    def test_list_filters_compose(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Match ongoing", priority="high")
            id2 = self.seed(d, "Match backlog", priority="high")
            id3 = self.seed(d, "Other ongoing", priority="high")
            self.t.cmd_mv(ns(dir=str(d), id=id1, status="ongoing", resolution=None))
            self.t.cmd_mv(ns(dir=str(d), id=id3, status="ongoing", resolution=None))
            out = self.listing(d, status="ongoing", match="match")
            self.assertIn(f"#{id1}", out)
            self.assertNotIn(f"#{id2}", out)     # filtered by status
            self.assertNotIn(f"#{id3}", out)     # filtered by match

    def paths(self, d, **over):
        """cmd_list in --paths output mode; filters default as in `listing`."""
        a = dict(dir=str(d), status=None, priority=None, label=None,
                 parent=None, match=None, sort=None, blocked=False, orphan=False,
                 flat=False, id=None, paths=True)
        a.update(over)
        return self.cap(self.t.cmd_list, ns(**a))

    def test_list_paths_emits_absolute_file_path_per_match(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            self.seed(d, "Beta")
            out = self.paths(d)
            lines = out.splitlines()
            self.assertEqual(len(lines), 2)
            for ln in lines:
                self.assertTrue(ln.startswith("/"))        # absolute
                self.assertTrue(ln.endswith(".md"))

    def test_list_paths_honors_status_filter(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Stay")
            id2 = self.seed(d, "Move")
            self.t.cmd_mv(ns(dir=str(d), id=id2, status="ongoing", resolution=None))
            out = self.paths(d, status="ongoing")
            lines = out.splitlines()
            self.assertEqual(len(lines), 1)
            self.assertTrue(lines[0].endswith(".md"))

    def test_list_paths_points_at_real_files(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Real")
            out = self.paths(d)
            for ln in out.splitlines():
                self.assertTrue(Path(ln).is_file())               # path actually resolves

    def test_list_paths_empty_when_no_match(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Only")
            self.assertEqual(self.paths(d, status="nonesuch"), "")

    def test_list_paths_excludes_nonmatching_ancestors(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic")
            self.seed(d, "Child")
            out = self.paths(d, match="child")
            lines = out.splitlines()
            self.assertEqual(len(lines), 1)               # only the match, no dim ancestor
            self.assertTrue(lines[0].endswith(".md"))

    def nested(self, d, **over):
        """cmd_list in its default nested-forest view; override per test."""
        a = dict(dir=str(d), status=None, priority=None, label=None,
                 parent=None, match=None, sort=None, blocked=False, orphan=False,
                 flat=False, id=None)
        a.update(over)
        return self.cap(self.t.cmd_list, ns(**a))

    def write_index(self, d, *objs):
        """Write a raw index.jsonl (for fixtures the verbs refuse to create, e.g.
        a dangling parent or a parent cycle)."""
        import json
        (d / "index.jsonl").write_text("\n".join(json.dumps(o) for o in objs) + "\n")

    def test_list_nested_by_default_indents_children(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Child", parent=id1)
            out = self.nested(d)
            self.assertIn("Epic", self.row_for(out, id1))
            self.assertIn("└─ Child", self.row_for(out, id2))   # sole child -> last branch
            self.assertNotIn("└─", self.row_for(out, id1))      # root carries no connector
            self.assertNotIn("├─", self.row_for(out, id1))

    def test_list_flat_has_no_connectors(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Child", parent=id1)
            out = self.listing(d)                          # flat=True
            self.assertIn(f"#{id1}", out)
            self.assertIn(f"#{id2}", out)
            self.assertNotIn("└─", out)
            self.assertNotIn("├─", out)

    def test_list_nested_child_omits_parent_pointer_tag(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Child", parent=id1)
            out = self.nested(d)
            self.assertNotIn("↳", self.row_for(out, id2))   # the connector already shows the parent

    def test_list_flat_child_keeps_parent_pointer_tag(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Child", parent=id1)
            out = self.listing(d)                          # flat: no indentation, so keep ↳
            self.assertIn(f"↳{id1}", self.row_for(out, id2))

    def test_list_positional_id_roots_subtree(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Child", parent=id1)
            id3 = self.seed(d, "Other")
            out = self.nested(d, id=id1)
            self.assertIn(f"#{id1}", out)
            self.assertIn(f"#{id2}", out)
            self.assertEqual("", self.row_for(out, id3))    # outside the subtree

    def test_list_filter_keeps_ancestor_spine(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Child", parent=id1)
            out = self.nested(d, match="child")
            self.assertIn(f"#{id1}", self.row_for(out, id1))    # spine kept as context
            self.assertIn("Child", self.row_for(out, id2))

    def test_list_filter_dims_nonmatching_ancestor(self):
        self.t._use_color = lambda: True
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Child", parent=id1)
            self.t.cmd_mv(ns(dir=str(d), id=id2, status="ongoing", resolution=None))
            out = self.nested(d, match="child")
            self.assertTrue(self.row_for(out, id1).startswith("\033[2m"))    # dimmed ancestor
            self.assertFalse(self.row_for(out, id2).startswith("\033[2m"))   # matched row colored

    def test_list_sort_orders_siblings_within_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Low child", parent=id1, priority="low")
            id3 = self.seed(d, "High child", parent=id1, priority="high")
            out = self.nested(d)                              # default id sort: both appear
            self.assertIn(f"#{id2}", out)
            self.assertIn(f"#{id3}", out)
            out = self.nested(d, sort="priority")             # high before low among siblings
            self.assertLess(out.index(f"#{id3}"), out.index(f"#{id2}"))

    def test_list_dangling_parent_renders_as_root(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.write_index(d, {"id": "2", "slug": "child", "title": "Orphanish",
                                 "status": "backlog",
                                 "priority": "high", "parent": "99"})
            out = self.nested(d)                              # parent 99 missing
            self.assertIn("#2", out)                          # promoted to a root, no crash
            self.assertNotIn("└─", self.row_for(out, 2))

    def test_list_parent_cycle_does_not_crash(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.write_index(d,
                {"id": "1", "slug": "a", "title": "A",
                 "status": "backlog", "priority": "high", "parent": "2"},
                {"id": "2", "slug": "b", "title": "B",
                 "status": "backlog", "priority": "high", "parent": "1"})
            out = self.nested(d)                              # must return, not hang/raise
            self.assertIsInstance(out, str)

    def test_tree_is_alias_for_list(self):
        parser = self.t.build_parser()
        flat = parser.parse_args(["list"])
        treed = parser.parse_args(["tree", "5"])
        self.assertIs(treed.func, flat.func)                 # tree dispatches to cmd_list
        self.assertEqual(treed.id, "5")                      # carries the positional id

    def test_tree_shows_children(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Child", parent=id1)
            out = self.nested(d)                              # tree is now the nested list
            self.assertIn("Epic", out)
            self.assertIn("Child", out)

    def test_list_argparse_exposes_flat_and_id(self):
        parser = self.t.build_parser()
        a = parser.parse_args(["list", "--flat", "3"])
        self.assertTrue(a.flat)
        self.assertEqual(a.id, "3")
        self.assertIs(a.func, self.t.cmd_list)

    def test_list_help_mentions_nested_and_flat(self):
        parser = self.t.build_parser()
        buf = io.StringIO()
        with redirect_stdout(buf), self.assertRaises(SystemExit):
            parser.parse_args(["list", "--help"])
        help_text = buf.getvalue()
        self.assertIn("nested", help_text)
        self.assertIn("--flat", help_text)

    def test_list_sort_orders_siblings_recursively(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Mid", parent=id1)
            id3 = self.seed(d, "Low grand", parent=id2, priority="low")
            id4 = self.seed(d, "High grand", parent=id2, priority="high")
            out = self.nested(d, sort="priority")
            # the sort reaches the grandchild sibling group, not just the top level
            self.assertLess(out.index(f"#{id4}"), out.index(f"#{id3}"))

    def test_list_dependency_cycle_does_not_crash(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.write_index(d,
                {"id": "1", "slug": "a", "title": "A",
                 "status": "backlog", "priority": "high", "depends_on": ["2"]},
                {"id": "2", "slug": "b", "title": "B",
                 "status": "backlog", "priority": "high", "depends_on": ["1"]})
            out = self.nested(d)                              # dep cycle: must render, not hang
            self.assertIn("#1", out)
            self.assertIn("#2", out)

    def test_ready_lists_unblocked_not_done_leaves(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Free")
            out = self.ready(d)
            self.assertIn(f"#{id1}", out)

    def test_ready_excludes_unmet_dep(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Dep")
            id2 = self.seed(d, "Blocked", depends=id1)
            out = self.ready(d)
            self.assertIn(f"#{id1}", out)                      # the dep itself is ready
            self.assertNotIn(f"#{id2}", out)

    def test_ready_includes_met_dep(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Dep")
            id2 = self.seed(d, "Unblocked", depends=id1)
            self.t.cmd_mv(ns(dir=str(d), id=id1, status="done", resolution=None))
            out = self.ready(d)
            self.assertNotIn(f"#{id1}", out)                   # done -> terminal, excluded
            self.assertIn(f"#{id2}", out)                      # its dep is now terminal

    def test_ready_excludes_parents(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic")
            id2 = self.seed(d, "Child", parent=id1)
            out = self.ready(d)
            self.assertNotIn(f"#{id1}", out)
            self.assertIn(f"#{id2}", out)

    def test_ready_excludes_terminal(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Done")
            self.t.cmd_mv(ns(dir=str(d), id=id1, status="done", resolution=None))
            self.assertEqual(self.ready(d), "")

    def test_ready_ordering_priority_then_points_then_id(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "High small", priority="high", points=1)
            id2 = self.seed(d, "High big", priority="high", points=8)
            id3 = self.seed(d, "Low big", priority="low", points=8)
            out = self.ready(d)
            self.assertLess(out.index(f"#{id2}"), out.index(f"#{id1}"))    # points within high
            self.assertLess(out.index(f"#{id1}"), out.index(f"#{id3}"))    # priority over points

    # `ready` ranks by demand, not by the declared priority alone: what an issue
    # unblocks counts as much as what it says it is (issue #yrre4zn).

    def test_ready_ranks_blocker_of_urgent_above_a_lone_high(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker", priority="medium")
            self.seed(d, "Urgent", priority="urgent", depends=blocker)
            lone = self.seed(d, "Lone high", priority="high")
            out = self.ready(d)
            self.assertLess(out.index(f"#{blocker}"), out.index(f"#{lone}"))

    def test_ready_ranks_by_how_many_are_blocked_at_the_same_level(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            two = self.seed(d, "Blocks two", priority="medium")
            one = self.seed(d, "Blocks one", priority="medium")
            self.seed(d, "H1", priority="high", depends=two)
            self.seed(d, "H2", priority="high", depends=two)
            self.seed(d, "H3", priority="high", depends=one)
            out = self.ready(d)
            self.assertLess(out.index(f"#{two}"), out.index(f"#{one}"))

    def test_ready_lifts_a_child_of_an_urgent_epic(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            epic = self.seed(d, "Urgent epic", priority="urgent")
            kid = self.seed(d, "Kid", priority="low", parent=epic)
            lone = self.seed(d, "Lone high", priority="high")
            out = self.ready(d)
            self.assertLess(out.index(f"#{kid}"), out.index(f"#{lone}"))

    def test_ready_keeps_points_then_id_tiebreaks_within_equal_demand(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            small = self.seed(d, "High small", priority="high", points=1)
            big = self.seed(d, "High big", priority="high", points=8)
            out = self.ready(d)
            self.assertLess(out.index(f"#{big}"), out.index(f"#{small}"))

    def test_ready_ignores_demand_from_terminal_dependents(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker", priority="medium")
            dead = self.seed(d, "Abandoned", priority="urgent", depends=blocker)
            lone = self.seed(d, "Lone high", priority="high")
            self.t.cmd_mv(ns(dir=str(d), id=dead, status="done", resolution="wontfix"))
            out = self.ready(d)
            self.assertLess(out.index(f"#{lone}"), out.index(f"#{blocker}"))

    # The marker is what keeps the ranking honest: without it `ready` shows a
    # medium above a high with no visible reason (issue #aujt85q).

    def test_ready_marks_an_inferred_row_with_the_culprit(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker", priority="medium")
            urgent = self.seed(d, "Urgent", priority="urgent", depends=blocker)
            out = self.row_for(self.ready(d), blocker)
            self.assertIn(f"↑urgent(#{urgent})", out)

    def test_ready_does_not_mark_a_row_that_is_its_own_maximum(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            top = self.seed(d, "Top", priority="urgent")
            self.seed(d, "Waiting", priority="high", depends=top)
            self.assertNotIn("↑", self.row_for(self.ready(d), top))

    def test_ready_marks_with_the_highest_priority_demander(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker", priority="lowest")
            self.seed(d, "Merely high", priority="high", depends=blocker)
            urgent = self.seed(d, "Urgent", priority="urgent", depends=blocker)
            row = self.row_for(self.ready(d), blocker)
            self.assertIn(f"↑urgent(#{urgent})", row)

    def test_ready_marks_a_child_of_a_hotter_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            epic = self.seed(d, "Urgent epic", priority="urgent")
            kid = self.seed(d, "Kid", priority="low", parent=epic)
            self.assertIn(f"↑urgent(#{epic})", self.row_for(self.ready(d), kid))

    def test_next_carries_the_marker(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker", priority="medium")
            urgent = self.seed(d, "Urgent", priority="urgent", depends=blocker)
            self.assertIn(f"↑urgent(#{urgent})", self.ready(d, next=True))

    def test_list_carries_no_demand_marker(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker", priority="medium")
            self.seed(d, "Urgent", priority="urgent", depends=blocker)
            self.assertNotIn("↑", self.listing(d))

    def test_next_prints_only_top_pick(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "High", priority="high")
            id2 = self.seed(d, "Low", priority="low")
            out = self.ready(d, next=True)
            self.assertIn(f"#{id1}", out)
            self.assertNotIn(f"#{id2}", out)

    def deps(self, d, issue_id=None, requires=False, blocks=False, full=False):
        """cmd_deps (graph is the only mode now); flags defaulted, override per test."""
        sid = str(issue_id) if issue_id is not None else None
        return self.cap(self.t.cmd_deps,
                        ns(dir=str(d), id=sid, requires=requires,
                           blocks=blocks, full=full))

    def test_deps_default_shows_both_cones(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "A")
            id2 = self.seed(d, "B")
            self.t.cmd_dep(ns(dir=str(d), id=id2, add=id1, remove=None))
            out = self.deps(d, id2)
            self.assertIn(f"#{id1}", out)                      # its prerequisite
            self.assertIn(f"#{id2}", out)

    def test_deps_default_includes_dependents(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Dep")
            id2 = self.seed(d, "X", depends=id1)
            id3 = self.seed(d, "Y", depends=id1)
            out = self.deps(d, id1)
            self.assertIn(f"#{id2}", out)                      # dependents appear
            self.assertIn(f"#{id3}", out)

    def test_deps_requires_scopes_to_prerequisite_cone(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Dep")
            id2 = self.seed(d, "Mid", depends=id1)
            id3 = self.seed(d, "Top", depends=id2)
            out = self.deps(d, id2, requires=True)
            self.assertIn(f"#{id1}", out)                      # its requirement (upstream)
            self.assertIn(f"#{id2}", out)
            self.assertNotIn(f"#{id3}", out)                   # dependent cone excluded

    def test_deps_blocks_scopes_to_dependent_cone(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Dep")
            id2 = self.seed(d, "Mid", depends=id1)
            id3 = self.seed(d, "Top", depends=id2)
            out = self.deps(d, id2, blocks=True)
            self.assertIn(f"#{id2}", out)
            self.assertIn(f"#{id3}", out)                      # the dependent (downstream)
            self.assertNotIn(f"#{id1}", out)                   # prerequisite cone excluded

    def test_deps_requires_cone_is_transitive(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Base")
            id2 = self.seed(d, "Mid", depends=id1)
            id3 = self.seed(d, "Top", depends=id2)
            out = self.deps(d, id3, requires=True)
            # the cone is transitive: both the direct and transitive requirement appear
            self.assertIn(f"#{id2}", out)
            self.assertIn(f"#{id1}", out)

    def test_deps_requires_without_id_is_an_error(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Solo")
            with self.assertRaises(SystemExit):
                self.deps(d, None, requires=True)          # cone flags need an id


class TestScopedReady(unittest.TestCase):
    """`ready`/`next` take an optional issue id and answer within that subtree: what
    can I pick up on *this* epic right now. Blocking stays effective — a leaf waiting
    on something outside the subtree, directly or through an ancestor's edge, is not
    ready no matter where the scope is drawn."""

    def setUp(self):
        self.t = load_trck()

    def cap(self, fn, args):
        buf = io.StringIO()
        with redirect_stdout(buf):
            fn(args)
        return buf.getvalue()

    def seed(self, d, title="Item", parent=None, priority="high", depends=None,
             points=None):
        a = ns(dir=str(d), title=title, priority=priority, parent=parent,
               points=points, depends=depends, spec=None, slug=None)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(a)
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def ready(self, d, issue_id=None, **over):
        a = dict(dir=str(d), next=False, id=issue_id)
        a.update(over)
        return self.cap(self.t.cmd_ready, ns(**a))

    def epic_with_two_kids(self, d):
        epic = self.seed(d, "Epic")
        return epic, self.seed(d, "Kid one", parent=epic), self.seed(d, "Kid two", parent=epic)

    # --- scoping ----------------------------------------------------------- #

    def test_scoping_ranks_over_the_whole_graph_then_filters(self):
        # The epic's medium kid blocks an urgent issue outside the scope; narrowing
        # the view must not hide the reason it outranks its high-priority sibling.
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            epic = self.seed(d, "Epic", priority="medium")
            kid = self.seed(d, "Blocker kid", parent=epic, priority="medium")
            sib = self.seed(d, "High sibling", parent=epic, priority="high")
            self.seed(d, "Urgent outsider", priority="urgent", depends=kid)
            out = self.ready(d, epic)
            self.assertLess(out.index(f"#{kid}"), out.index(f"#{sib}"))

    def test_scoping_keeps_only_the_subtree(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            epic, kid1, kid2 = self.epic_with_two_kids(d)
            outside = self.seed(d, "Unrelated")
            out = self.ready(d, epic)
            self.assertIn(f"#{kid1}", out)
            self.assertIn(f"#{kid2}", out)
            self.assertNotIn(f"#{outside}", out)

    def test_without_an_id_nothing_changes(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            _epic, kid1, _kid2 = self.epic_with_two_kids(d)
            outside = self.seed(d, "Unrelated")
            out = self.ready(d)
            self.assertIn(f"#{kid1}", out)
            self.assertIn(f"#{outside}", out)

    def test_the_parent_itself_is_never_listed(self):
        # it has children, so it is not a leaf and there is nothing to pick up on it
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            epic, _k1, _k2 = self.epic_with_two_kids(d)
            self.assertNotIn(f"#{epic}", self.ready(d, epic))

    def test_a_grandchild_is_in_scope(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            epic = self.seed(d, "Epic")
            mid = self.seed(d, "Middle", parent=epic)
            leaf = self.seed(d, "Leaf", parent=mid)
            self.assertIn(f"#{leaf}", self.ready(d, epic))

    def test_an_id_prefix_resolves(self):
        # Ids are random, so slicing a fixed number of characters off one is not
        # reliably unambiguous — two of three ids sharing their first two chars is
        # rare, not impossible, and made this flake. Pin the ids instead, so "ab"
        # can only mean the epic and the assertion is about resolution, not luck.
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            ids = iter(["abcdefg", "mnpqrst", "wxyz234"])
            with mock.patch.object(self.t, "gen_id", lambda ctx: next(ids)):
                epic, kid1, _kid2 = self.epic_with_two_kids(d)
            self.assertEqual((epic, kid1), ("abcdefg", "mnpqrst"))
            self.assertIn(f"#{kid1}", self.ready(d, "ab"))

    # --- blocking stays effective ------------------------------------------ #

    def test_a_blocker_outside_the_subtree_still_blocks(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker")
            epic = self.seed(d, "Epic")
            kid = self.seed(d, "Kid", parent=epic, depends=blocker)
            free = self.seed(d, "Free kid", parent=epic)
            out = self.ready(d, epic)
            self.assertNotIn(f"#{kid}", out)         # waiting on work outside the scope
            self.assertIn(f"#{free}", out)

    def test_a_blocker_inherited_from_the_parent_still_blocks(self):
        # the edge is authored on the epic, so every child waits on it — scoping to
        # the epic must not make its children look actionable
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker")
            epic = self.seed(d, "Epic", depends=blocker)
            kid = self.seed(d, "Kid", parent=epic)
            self.assertNotIn(f"#{kid}", self.ready(d, epic))

    def test_scoping_to_a_ready_leaf_yields_that_leaf(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            _epic, kid1, kid2 = self.epic_with_two_kids(d)
            out = self.ready(d, kid1)
            self.assertIn(f"#{kid1}", out)
            self.assertNotIn(f"#{kid2}", out)

    def test_scoping_to_a_blocked_leaf_yields_nothing(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            blocker = self.seed(d, "Blocker")
            kid = self.seed(d, "Blocked", depends=blocker)
            self.assertNotIn(f"#{kid}", self.ready(d, kid))

    # --- next --------------------------------------------------------------- #

    def test_next_picks_the_best_within_the_subtree(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Urgent elsewhere", priority="urgent")
            epic = self.seed(d, "Epic")
            self.seed(d, "Low kid", parent=epic, priority="low")
            top = self.seed(d, "Medium kid", parent=epic, priority="medium")
            out = self.cap(self.t.cmd_next, ns(dir=str(d), id=epic))
            self.assertIn(f"#{top}", out)
            self.assertEqual(len(out.strip().splitlines()), 1)

    def test_next_without_an_id_still_spans_the_tracker(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            urgent = self.seed(d, "Urgent elsewhere", priority="urgent")
            epic = self.seed(d, "Epic")
            self.seed(d, "Low kid", parent=epic, priority="low")
            out = self.cap(self.t.cmd_next, ns(dir=str(d), id=None))
            self.assertIn(f"#{urgent}", out)
