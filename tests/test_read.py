import io
import unittest
from contextlib import redirect_stdout
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
        """cmd_list with all current flags defaulted; override per test."""
        a = dict(dir=str(d), status=None, kind=None, priority=None, label=None,
                 parent=None, match=None, sort=None, blocked=False, orphan=False)
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
            self.assertIn('"id": 1', out)
            self.assertIn("--- body ---", out)

    def test_list_filters_by_status(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            self.seed(d, "B")
            self.t.cmd_mv(ns(dir=str(d), id=2, status="ongoing", resolution=None))
            out = self.cap(self.t.cmd_list, ns(dir=str(d), status="ongoing",
                                               kind=None, priority=None, parent=None))
            self.assertIn("#002", out)
            self.assertNotIn("#001", out)

    def test_list_filters_by_parent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")     # id 1
            self.seed(d, "Child", parent=1)        # id 2
            self.seed(d, "Loose")                  # id 3, no parent
            out = self.cap(self.t.cmd_list, ns(dir=str(d), status=None,
                                               kind=None, priority=None, parent=1))
            self.assertIn("#002", out)
            self.assertNotIn("#001", out)
            self.assertNotIn("#003", out)

    def test_list_status_multi_and_negated(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")                                   # 1 backlog
            self.seed(d, "B")                                   # 2 -> ongoing
            self.seed(d, "C")                                   # 3 -> done
            self.t.cmd_mv(ns(dir=str(d), id=2, status="ongoing", resolution=None))
            self.t.cmd_mv(ns(dir=str(d), id=3, status="done", resolution=None))
            out = self.listing(d, status="backlog,ongoing")
            self.assertIn("#001", out)
            self.assertIn("#002", out)
            self.assertNotIn("#003", out)
            out = self.listing(d, status="!done")
            self.assertIn("#001", out)
            self.assertIn("#002", out)
            self.assertNotIn("#003", out)

    def test_list_match_is_case_insensitive_substring(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Fix the parser")
            self.seed(d, "Add a feature")
            out = self.listing(d, match="PARSER")
            self.assertIn("#001", out)
            self.assertNotIn("#002", out)

    def test_list_sort_by_priority(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Low one", priority="low")        # 1
            self.seed(d, "High one", priority="high")      # 2
            self.seed(d, "Mid one", priority="medium")     # 3
            out = self.listing(d, sort="priority")
            self.assertLess(out.index("#002"), out.index("#003"))
            self.assertLess(out.index("#003"), out.index("#001"))

    def test_list_sort_by_points(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Small", points=1)    # 1
            self.seed(d, "Big", points=8)      # 2
            self.seed(d, "Mid", points=3)      # 3
            out = self.listing(d, sort="points")
            self.assertLess(out.index("#002"), out.index("#003"))
            self.assertLess(out.index("#003"), out.index("#001"))

    def test_list_blocked_only(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1 backlog (non-terminal)
            self.seed(d, "Blocked", depends="1")           # 2 depends on open #1
            self.seed(d, "Free")                           # 3 no deps
            out = self.listing(d, blocked=True)
            self.assertIn("#002", out)
            self.assertNotIn("#001", out)
            self.assertNotIn("#003", out)
            # once the dependency is terminal, nothing is blocked
            self.t.cmd_mv(ns(dir=str(d), id=1, status="done", resolution=None))
            self.assertEqual(self.listing(d, blocked=True), "")

    def test_list_orphan_only(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")              # 1 top-level
            self.seed(d, "Child", parent=1)                # 2 has parent
            out = self.listing(d, orphan=True)
            self.assertIn("#001", out)
            self.assertNotIn("#002", out)

    def test_list_filters_compose(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Match ongoing", priority="high")     # 1 -> ongoing
            self.seed(d, "Match backlog", priority="high")     # 2 stays backlog
            self.seed(d, "Other ongoing", priority="high")     # 3 -> ongoing
            self.t.cmd_mv(ns(dir=str(d), id=1, status="ongoing", resolution=None))
            self.t.cmd_mv(ns(dir=str(d), id=3, status="ongoing", resolution=None))
            out = self.listing(d, status="ongoing", match="match")
            self.assertIn("#001", out)
            self.assertNotIn("#002", out)     # filtered by status
            self.assertNotIn("#003", out)     # filtered by match

    def test_tree_shows_children(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")
            self.seed(d, "Child", parent=1)
            out = self.cap(self.t.cmd_tree, ns(dir=str(d), id=None))
            self.assertIn("Epic", out)
            self.assertIn("Child", out)

    def test_ready_lists_unblocked_not_done_leaves(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Free")                           # 1 leaf, no deps
            out = self.ready(d)
            self.assertIn("#001", out)

    def test_ready_excludes_unmet_dep(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1 backlog (non-terminal)
            self.seed(d, "Blocked", depends="1")           # 2 unmet dep
            out = self.ready(d)
            self.assertIn("#001", out)                     # the dep itself is ready
            self.assertNotIn("#002", out)

    def test_ready_includes_met_dep(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep")                            # 1
            self.seed(d, "Unblocked", depends="1")         # 2
            self.t.cmd_mv(ns(dir=str(d), id=1, status="done", resolution=None))
            out = self.ready(d)
            self.assertNotIn("#001", out)                  # done -> terminal, excluded
            self.assertIn("#002", out)                     # its dep is now terminal

    def test_ready_excludes_parents(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")              # 1 non-leaf (has child)
            self.seed(d, "Child", parent=1)                # 2 leaf
            out = self.ready(d)
            self.assertNotIn("#001", out)
            self.assertIn("#002", out)

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
            self.assertLess(out.index("#002"), out.index("#001"))   # points within high
            self.assertLess(out.index("#001"), out.index("#003"))   # priority over points

    def test_next_prints_only_top_pick(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "High", priority="high")          # 1
            self.seed(d, "Low", priority="low")            # 2
            out = self.ready(d, next=True)
            self.assertIn("#001", out)
            self.assertNotIn("#002", out)

    def test_deps_shows_requires_and_blocks(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            self.seed(d, "B")
            self.t.cmd_dep(ns(dir=str(d), id=2, add=1, remove=None))
            out = self.cap(self.t.cmd_deps, ns(dir=str(d), id=2,
                                               requires=False, blocks=False))
            self.assertIn("requires", out)
            self.assertIn("#001", out)
