import io
import unittest
from contextlib import redirect_stdout
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestRead(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title="Item", kind=None, parent=None):
        a = ns(dir=str(d), title=title, priority="high", kind=kind, parent=parent,
               depends=None, spec=None, slug=None)
        self.t.cmd_new(a)

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

    def test_tree_shows_children(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", kind="epic")
            self.seed(d, "Child", parent=1)
            out = self.cap(self.t.cmd_tree, ns(dir=str(d), id=None))
            self.assertIn("Epic", out)
            self.assertIn("Child", out)

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
