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
        self.t.cmd_new(a)

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
            self.seed(d, "Hello")
            out = self.cap(self.t.cmd_show, ns(dir=str(d), id=1, json=False))
            self.assertIn("title", out)        # aligned key: value, not raw JSON
            self.assertIn("Hello", out)
            self.assertNotIn('"id": 1', out)
            self.assertIn("--- body ---", out)
            self.assertIn("# Hello", out)

    def test_show_json_flag(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Hello")
            out = self.cap(self.t.cmd_show, ns(dir=str(d), id=1, json=True))
            self.assertIn('"id": "1"', out)
            self.assertIn("--- body ---", out)

    def test_list_filters_by_status(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            self.seed(d, "B")
            self.t.cmd_mv(ns(dir=str(d), id=2, status="ongoing", resolution=None))
            out = self.cap(self.t.cmd_list, ns(dir=str(d), status="ongoing",
                                               kind=None, priority=None, parent=None,
                                               flat=True, id=None))
            self.assertIn("#2", out)
            self.assertNotIn("#1", out)

    def test_list_filters_by_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")     # id 1
            self.seed(d, "Child", parent=1)        # id 2
            self.seed(d, "Loose")                  # id 3, no parent
            out = self.cap(self.t.cmd_list, ns(dir=str(d), status=None,
                                               kind=None, priority=None, parent=1,
                                               flat=True, id=None))
            self.assertIn("#2", out)
            self.assertNotIn("#1", out)
            self.assertNotIn("#3", out)

    def test_list_status_multi_and_negated(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")                                   # 1 backlog
            self.seed(d, "B")                                   # 2 -> ongoing
            self.seed(d, "C")                                   # 3 -> done
            self.t.cmd_mv(ns(dir=str(d), id=2, status="ongoing", resolution=None))
            self.t.cmd_mv(ns(dir=str(d), id=3, status="done", resolution=None))
            out = self.listing(d, status="backlog,ongoing")
            self.assertIn("#1", out)
            self.assertIn("#2", out)
            self.assertNotIn("#3", out)
            out = self.listing(d, status="!done")
            self.assertIn("#1", out)
            self.assertIn("#2", out)
            self.assertNotIn("#3", out)

    def test_list_match_is_case_insensitive_substring(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Fix the parser")
            self.seed(d, "Add a feature")
            out = self.listing(d, match="PARSER")
            self.assertIn("#1", out)
            self.assertNotIn("#2", out)

    def test_list_sort_by_priority(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Low one", priority="low")        # 1
            self.seed(d, "High one", priority="high")      # 2
            self.seed(d, "Mid one", priority="medium")     # 3
            out = self.listing(d, sort="priority")
            self.assertLess(out.index("#2"), out.index("#3"))
            self.assertLess(out.index("#3"), out.index("#1"))

    def test_list_sort_by_points(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Small", points=1)    # 1
            self.seed(d, "Big", points=8)      # 2
            self.seed(d, "Mid", points=3)      # 3
            out = self.listing(d, sort="points")
            self.assertLess(out.index("#2"), out.index("#3"))
            self.assertLess(out.index("#3"), out.index("#1"))

    def test_list_blocked_only(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1 backlog (non-terminal)
            self.seed(d, "Blocked", depends="1")           # 2 depends on open #1
            self.seed(d, "Free")                           # 3 no deps
            out = self.listing(d, blocked=True)
            self.assertIn("#2", out)
            # only #2 is listed as a row; #1 appears only inside its `needs` note
            self.assertEqual("", self.row_for(out, 1))
            self.assertEqual("", self.row_for(out, 3))
            # once the dependency is terminal, nothing is blocked
            self.t.cmd_mv(ns(dir=str(d), id=1, status="done", resolution=None))
            self.assertEqual(self.listing(d, blocked=True), "")

    @staticmethod
    def row_for(out, issue_id):
        """The output line whose OWN id (the first #NNN on the line, the id column) is
        `issue_id` — not a line that merely mentions it in a needs/blocks annotation."""
        tok = f"#{issue_id}"
        for ln in out.splitlines():
            m = re.search(r"#\d+", ln)
            if m and m.group(0) == tok:
                return ln
        return ""

    def test_list_annotates_blocked_row_with_needs(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1 backlog (non-terminal)
            self.seed(d, "Blocked", depends="1")           # 2 depends on open #1
            out = self.listing(d)
            self.assertIn("needs #1", self.row_for(out, 2))

    def test_list_needs_omits_terminal_blocker(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1
            self.seed(d, "Blocked", depends="1")           # 2
            self.t.cmd_mv(ns(dir=str(d), id=1, status="done", resolution=None))
            out = self.listing(d)
            # block cleared: the dependent no longer advertises a blocker
            self.assertNotIn("needs", self.row_for(out, 2))

    def test_list_needs_lists_only_open_blockers(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "DepA")                           # 1
            self.seed(d, "DepB")                           # 2
            self.seed(d, "Blocked", depends="1,2")         # 3 depends on both
            self.t.cmd_mv(ns(dir=str(d), id=1, status="done", resolution=None))
            line = self.row_for(self.listing(d), 3)
            self.assertIn("needs #2", line)               # still open
            self.assertNotIn("#1", line)                  # done -> dropped

    def test_list_annotates_blocker_row_with_blocks(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1 (the blocker)
            self.seed(d, "Blocked", depends="1")           # 2 depends on #1
            line = self.row_for(self.listing(d), 1)
            self.assertIn("blocks #2", line)

    def test_list_blocks_cleared_when_blocker_done(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1
            self.seed(d, "Blocked", depends="1")           # 2
            self.t.cmd_mv(ns(dir=str(d), id=1, status="done", resolution=None))
            out = self.listing(d)
            # a done task blocks nothing — both sides of the edge go quiet
            self.assertNotIn("blocks", self.row_for(out, 1))
            self.assertNotIn("needs", self.row_for(out, 2))

    def test_list_block_annotations_are_plain_without_color(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1
            self.seed(d, "Blocked", depends="1")           # 2
            out = self.listing(d)
            self.assertIn("needs #1", out)
            self.assertNotIn("\x1b[", out)                 # no ANSI when color is off

    def test_ready_has_no_block_annotations(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1 ready, blocks #2
            self.seed(d, "Blocked", depends="1")           # 2 blocked (absent from ready)
            out = self.ready(d)
            self.assertIn("#1", out)
            self.assertNotIn("blocks", out)                # ready stays terse
            self.assertNotIn("needs", out)

    def test_list_orphan_only(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")              # 1 top-level
            self.seed(d, "Child", parent=1)                # 2 has parent
            out = self.listing(d, orphan=True)
            self.assertIn("#1", out)
            self.assertNotIn("#2", out)

    def test_list_filters_compose(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Match ongoing", priority="high")     # 1 -> ongoing
            self.seed(d, "Match backlog", priority="high")     # 2 stays backlog
            self.seed(d, "Other ongoing", priority="high")     # 3 -> ongoing
            self.t.cmd_mv(ns(dir=str(d), id=1, status="ongoing", resolution=None))
            self.t.cmd_mv(ns(dir=str(d), id=3, status="ongoing", resolution=None))
            out = self.listing(d, status="ongoing", match="match")
            self.assertIn("#1", out)
            self.assertNotIn("#2", out)     # filtered by status
            self.assertNotIn("#3", out)     # filtered by match

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
            self.seed(d, "Alpha")          # 1 -> 001-alpha.md
            self.seed(d, "Beta")           # 2 -> 002-beta.md
            out = self.paths(d)
            lines = out.splitlines()
            self.assertEqual(len(lines), 2)
            for ln in lines:
                self.assertTrue(ln.startswith("/"))        # absolute
                self.assertTrue(ln.endswith(".md"))
            self.assertTrue(any(ln.endswith("001-alpha.md") for ln in lines))
            self.assertTrue(any(ln.endswith("002-beta.md") for ln in lines))

    def test_list_paths_honors_status_filter(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Stay")                                   # 1 backlog
            self.seed(d, "Move")                                   # 2 -> ongoing
            self.t.cmd_mv(ns(dir=str(d), id=2, status="ongoing", resolution=None))
            out = self.paths(d, status="ongoing")
            lines = out.splitlines()
            self.assertEqual(len(lines), 1)
            self.assertTrue(lines[0].endswith("002-move.md"))

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
            self.seed(d, "Epic", kind="epic")             # 1 ancestor, does NOT match
            self.seed(d, "Child", parent=1)               # 2 matches
            out = self.paths(d, match="child")
            lines = out.splitlines()
            self.assertEqual(len(lines), 1)               # only the match, no dim ancestor
            self.assertTrue(lines[0].endswith("002-child.md"))

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
            self.seed(d, "Epic", kind="epic")             # 1 root
            self.seed(d, "Child", parent=1)               # 2 child
            out = self.nested(d)
            self.assertIn("Epic", self.row_for(out, 1))
            self.assertIn("└─ Child", self.row_for(out, 2))   # sole child -> last branch
            self.assertNotIn("└─", self.row_for(out, 1))      # root carries no connector
            self.assertNotIn("├─", self.row_for(out, 1))

    def test_list_flat_has_no_connectors(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")
            self.seed(d, "Child", parent=1)
            out = self.listing(d)                          # flat=True
            self.assertIn("#1", out)
            self.assertIn("#2", out)
            self.assertNotIn("└─", out)
            self.assertNotIn("├─", out)

    def test_list_nested_child_omits_parent_pointer_tag(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")
            self.seed(d, "Child", parent=1)
            out = self.nested(d)
            self.assertNotIn("↳", self.row_for(out, 2))   # the connector already shows the parent

    def test_list_flat_child_keeps_parent_pointer_tag(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")
            self.seed(d, "Child", parent=1)
            out = self.listing(d)                          # flat: no indentation, so keep ↳
            self.assertIn("↳1", self.row_for(out, 2))

    def test_list_positional_id_roots_subtree(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")             # 1
            self.seed(d, "Child", parent=1)               # 2
            self.seed(d, "Other")                         # 3 unrelated
            out = self.nested(d, id=1)
            self.assertIn("#1", out)
            self.assertIn("#2", out)
            self.assertEqual("", self.row_for(out, 3))    # outside the subtree

    def test_list_filter_keeps_ancestor_spine(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")             # 1 ancestor (does not match)
            self.seed(d, "Child", parent=1)               # 2 matches
            out = self.nested(d, match="child")
            self.assertIn("#1", self.row_for(out, 1))    # spine kept as context
            self.assertIn("Child", self.row_for(out, 2))

    def test_list_filter_dims_nonmatching_ancestor(self):
        self.t._use_color = lambda: True
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")             # 1 ancestor, stays backlog
            self.seed(d, "Child", parent=1)               # 2
            self.t.cmd_mv(ns(dir=str(d), id=2, status="ongoing", resolution=None))
            out = self.nested(d, match="child")
            self.assertTrue(self.row_for(out, 1).startswith("\033[2m"))    # dimmed ancestor
            self.assertFalse(self.row_for(out, 2).startswith("\033[2m"))   # matched row colored

    def test_list_sort_orders_siblings_within_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")                 # 1
            self.seed(d, "Low child", parent=1, priority="low")    # 2
            self.seed(d, "High child", parent=1, priority="high")  # 3
            out = self.nested(d)                              # default id
            self.assertLess(out.index("#2"), out.index("#3"))
            out = self.nested(d, sort="priority")             # high before low among siblings
            self.assertLess(out.index("#3"), out.index("#2"))

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
            self.seed(d, "Epic", kind="epic")
            self.seed(d, "Child", parent=1)
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
            self.seed(d, "Epic", kind="epic")                      # 1
            self.seed(d, "Mid", parent=1)                          # 2 (has its own children)
            self.seed(d, "Low grand", parent=2, priority="low")    # 3
            self.seed(d, "High grand", parent=2, priority="high")  # 4
            out = self.nested(d, sort="priority")
            # the sort reaches the grandchild sibling group, not just the top level
            self.assertLess(out.index("#4"), out.index("#3"))

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
            self.seed(d, "Free")                           # 1 leaf, no deps
            out = self.ready(d)
            self.assertIn("#1", out)

    def test_ready_excludes_unmet_dep(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1 backlog (non-terminal)
            self.seed(d, "Blocked", depends="1")           # 2 unmet dep
            out = self.ready(d)
            self.assertIn("#1", out)                      # the dep itself is ready
            self.assertNotIn("#2", out)

    def test_ready_includes_met_dep(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1
            self.seed(d, "Unblocked", depends="1")         # 2
            self.t.cmd_mv(ns(dir=str(d), id=1, status="done", resolution=None))
            out = self.ready(d)
            self.assertNotIn("#1", out)                   # done -> terminal, excluded
            self.assertIn("#2", out)                      # its dep is now terminal

    def test_ready_excludes_parents(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")              # 1 non-leaf (has child)
            self.seed(d, "Child", parent=1)                # 2 leaf
            out = self.ready(d)
            self.assertNotIn("#1", out)
            self.assertIn("#2", out)

    def test_ready_excludes_terminal(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Done")                           # 1
            self.t.cmd_mv(ns(dir=str(d), id=1, status="done", resolution=None))
            self.assertEqual(self.ready(d), "")

    def test_ready_ordering_priority_then_points_then_id(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "High small", priority="high", points=1)   # 1
            self.seed(d, "High big", priority="high", points=8)     # 2
            self.seed(d, "Low big", priority="low", points=8)       # 3
            out = self.ready(d)
            self.assertLess(out.index("#2"), out.index("#1"))    # points within high
            self.assertLess(out.index("#1"), out.index("#3"))    # priority over points

    def test_next_prints_only_top_pick(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "High", priority="high")          # 1
            self.seed(d, "Low", priority="low")            # 2
            out = self.ready(d, next=True)
            self.assertIn("#1", out)
            self.assertNotIn("#2", out)

    def deps(self, d, issue_id=None, requires=False, blocks=False, full=False):
        """cmd_deps (graph is the only mode now); flags defaulted, override per test."""
        sid = str(issue_id) if issue_id is not None else None
        return self.cap(self.t.cmd_deps,
                        ns(dir=str(d), id=sid, requires=requires,
                           blocks=blocks, full=full))

    def test_deps_default_shows_both_cones(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            self.seed(d, "B")
            self.t.cmd_dep(ns(dir=str(d), id=2, add=1, remove=None))
            out = self.deps(d, 2)                          # neither flag: full line
            self.assertIn("#1", out)                      # its prerequisite
            self.assertIn("#2", out)

    def test_deps_default_includes_dependents(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1 (the blocker)
            self.seed(d, "X", depends="1")                 # 2 depends on 1
            self.seed(d, "Y", depends="1")                 # 3 depends on 1
            out = self.deps(d, 1)                          # default: whole line
            self.assertIn("#2", out)                      # dependents appear
            self.assertIn("#3", out)

    def test_deps_requires_scopes_to_prerequisite_cone(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1
            self.seed(d, "Mid", depends="1")               # 2 requires 1, blocks 3
            self.seed(d, "Top", depends="2")               # 3
            out = self.deps(d, 2, requires=True)
            self.assertIn("#1", out)                      # its requirement (upstream)
            self.assertIn("#2", out)
            self.assertNotIn("#3", out)                   # dependent cone excluded

    def test_deps_blocks_scopes_to_dependent_cone(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1
            self.seed(d, "Mid", depends="1")               # 2 requires 1, blocks 3
            self.seed(d, "Top", depends="2")               # 3
            out = self.deps(d, 2, blocks=True)
            self.assertIn("#2", out)
            self.assertIn("#3", out)                      # the dependent (downstream)
            self.assertNotIn("#1", out)                   # prerequisite cone excluded

    def test_deps_requires_cone_is_transitive(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Base")                           # 1
            self.seed(d, "Mid", depends="1")               # 2 requires 1
            self.seed(d, "Top", depends="2")               # 3 requires 2 (-> 1)
            out = self.deps(d, 3, requires=True)
            # the cone is transitive: both the direct and transitive requirement appear
            self.assertIn("#2", out)
            self.assertIn("#1", out)

    def test_deps_requires_without_id_is_an_error(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Solo")
            with self.assertRaises(SystemExit):
                self.deps(d, None, requires=True)          # cone flags need an id
