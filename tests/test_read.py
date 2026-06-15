import io
import re
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestRead(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title="Item", kind=None, parent=None, priority="high",
             points=None, depends=None):
        a = ns(dir=str(d), title=title, priority=priority, kind=kind, parent=parent,
               points=points, depends=depends, spec=None, slug=None)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(a)
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def listing(self, d, **over):
        """cmd_list defaulted to the flat view (the stable regression baseline);
        override `flat=False` for nested-forest tests."""
        a = dict(dir=str(d), status=None, kind=None, priority=None, label=None,
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
            self.assertNotIn('"id": 1', out)
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
                                               kind=None, priority=None, parent=None,
                                               flat=True, id=None))
            self.assertIn(f"#{id2}", out)
            self.assertNotIn(f"#{id1}", out)

    def test_list_filters_by_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic", kind="epic")
            id2 = self.seed(d, "Child", parent=id1)
            id3 = self.seed(d, "Loose")
            out = self.cap(self.t.cmd_list, ns(dir=str(d), status=None,
                                               kind=None, priority=None, parent=id1,
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

    def test_list_orphan_only(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic", kind="epic")
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
        a = dict(dir=str(d), status=None, kind=None, priority=None, label=None,
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
            self.seed(d, "Epic", kind="epic")
            self.seed(d, "Child")
            out = self.paths(d, match="child")
            lines = out.splitlines()
            self.assertEqual(len(lines), 1)               # only the match, no dim ancestor
            self.assertTrue(lines[0].endswith(".md"))

    def nested(self, d, **over):
        """cmd_list in its default nested-forest view; override per test."""
        a = dict(dir=str(d), status=None, kind=None, priority=None, label=None,
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
            id1 = self.seed(d, "Epic", kind="epic")
            id2 = self.seed(d, "Child", parent=id1)
            out = self.nested(d)
            self.assertIn("Epic", self.row_for(out, id1))
            self.assertIn("└─ Child", self.row_for(out, id2))   # sole child -> last branch
            self.assertNotIn("└─", self.row_for(out, id1))      # root carries no connector
            self.assertNotIn("├─", self.row_for(out, id1))

    def test_list_flat_has_no_connectors(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic", kind="epic")
            id2 = self.seed(d, "Child", parent=id1)
            out = self.listing(d)                          # flat=True
            self.assertIn(f"#{id1}", out)
            self.assertIn(f"#{id2}", out)
            self.assertNotIn("└─", out)
            self.assertNotIn("├─", out)

    def test_list_nested_child_omits_parent_pointer_tag(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic", kind="epic")
            id2 = self.seed(d, "Child", parent=id1)
            out = self.nested(d)
            self.assertNotIn("↳", self.row_for(out, id2))   # the connector already shows the parent

    def test_list_flat_child_keeps_parent_pointer_tag(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic", kind="epic")
            id2 = self.seed(d, "Child", parent=id1)
            out = self.listing(d)                          # flat: no indentation, so keep ↳
            self.assertIn(f"↳{id1}", self.row_for(out, id2))

    def test_list_positional_id_roots_subtree(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic", kind="epic")
            id2 = self.seed(d, "Child", parent=id1)
            id3 = self.seed(d, "Other")
            out = self.nested(d, id=id1)
            self.assertIn(f"#{id1}", out)
            self.assertIn(f"#{id2}", out)
            self.assertEqual("", self.row_for(out, id3))    # outside the subtree

    def test_list_filter_keeps_ancestor_spine(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic", kind="epic")
            id2 = self.seed(d, "Child", parent=id1)
            out = self.nested(d, match="child")
            self.assertIn(f"#{id1}", self.row_for(out, id1))    # spine kept as context
            self.assertIn("Child", self.row_for(out, id2))

    def test_list_filter_dims_nonmatching_ancestor(self):
        self.t._use_color = lambda: True
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic", kind="epic")
            id2 = self.seed(d, "Child", parent=id1)
            self.t.cmd_mv(ns(dir=str(d), id=id2, status="ongoing", resolution=None))
            out = self.nested(d, match="child")
            self.assertTrue(self.row_for(out, id1).startswith("\033[2m"))    # dimmed ancestor
            self.assertFalse(self.row_for(out, id2).startswith("\033[2m"))   # matched row colored

    def test_list_sort_orders_siblings_within_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, "Epic", kind="epic")
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
            self.write_index(d, {"id": 2, "slug": "child", "title": "Orphanish",
                                 "kind": "task", "status": "backlog",
                                 "priority": "high", "parent": 99})
            out = self.nested(d)                              # parent 99 missing
            self.assertIn("#2", out)                          # promoted to a root, no crash
            self.assertNotIn("└─", self.row_for(out, 2))

    def test_list_parent_cycle_does_not_crash(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.write_index(d,
                {"id": 1, "slug": "a", "title": "A", "kind": "task",
                 "status": "backlog", "priority": "high", "parent": 2},
                {"id": 2, "slug": "b", "title": "B", "kind": "task",
                 "status": "backlog", "priority": "high", "parent": 1})
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
            id1 = self.seed(d, "Epic", kind="epic")
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
            id1 = self.seed(d, "Epic", kind="epic")
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
                {"id": 1, "slug": "a", "title": "A", "kind": "task",
                 "status": "backlog", "priority": "high", "depends_on": [2]},
                {"id": 2, "slug": "b", "title": "B", "kind": "task",
                 "status": "backlog", "priority": "high", "depends_on": [1]})
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
            id1 = self.seed(d, "Epic", kind="epic")
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
