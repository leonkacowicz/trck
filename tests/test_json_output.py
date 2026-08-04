"""`--json` on the read commands, and the one seam they all go through.

The point of a shared `emit_json` is that a consumer parsing `list --json` and
`show --json` should not have to notice which command produced the bytes: same
encoder options, same trailing newline, same issue shape from `Issue.to_dict()`.
Without a seam that drifts one command at a time and nothing catches it."""
import io
import json
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class JsonBase(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title, **over):
        args = ns(dir=str(d), title=title, priority=over.pop("priority", "medium"),
                  points=over.pop("points", None), parent=over.pop("parent", None),
                  depends=over.pop("depends", None), spec=None, slug=None,
                  review_url=None, id=over.pop("id", None))
        with redirect_stdout(io.StringIO()):
            self.t.cmd_new(args)

    def capture(self, fn, **kw):
        buf = io.StringIO()
        with redirect_stdout(buf):
            fn(ns(**kw))
        return buf.getvalue()


class TestEmitJson(JsonBase):
    def test_it_is_indented_utf8_with_a_trailing_newline(self):
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.emit_json({"title": "café", "n": [1, 2]})
        out = buf.getvalue()
        self.assertTrue(out.endswith("\n"))
        self.assertIn("café", out)          # ensure_ascii=False, not é
        self.assertIn("\n  ", out)          # indent=2
        self.assertEqual(json.loads(out), {"title": "café", "n": [1, 2]})

    def test_it_emits_exactly_one_document(self):
        """Every consumer of these commands is `json.loads(stdout)`. Two documents on
        stdout — which `show --json` used to produce, metadata then a body separator —
        means the obvious way to consume it fails."""
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.emit_json([1, 2, 3])
        self.assertEqual(len(buf.getvalue().rstrip("\n").split("\n\n")), 1)
        json.loads(buf.getvalue())          # parses whole, not line by line

    def test_an_empty_result_is_still_a_document(self):
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.emit_json([])
        self.assertEqual(json.loads(buf.getvalue()), [])


class TestShowJson(JsonBase):
    def test_it_is_one_document_with_the_body_folded_in(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha", id="aaaaaaa")
            out = self.capture(self.t.cmd_show, dir=str(d), id="aaaaaaa", json=True)
            doc = json.loads(out)           # parses whole — no `--- body ---` trailer
            self.assertEqual(doc["id"], "aaaaaaa")
            self.assertEqual(doc["title"], "Alpha")
            self.assertIn("# Alpha", doc["body"])
            self.assertNotIn("--- body ---", out)

    def test_the_body_is_the_raw_file_contents(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha", id="aaaaaaa")
            path = next((Path(d) / "items").glob("aaaaaaa-*.md"))
            path.write_text("# Alpha\n\nHand-written *prose*, `code`, and a #hash.\n")
            doc = json.loads(self.capture(self.t.cmd_show, dir=str(d), id="aaaaaaa",
                                          json=True))
            self.assertEqual(doc["body"], path.read_text())

    def test_a_non_leaf_omits_points_as_the_human_view_does(self):
        """Points roll up from leaves, so on a parent the stored value is not an input.
        The human view hides it; the JSON has to agree or the two disagree about what
        the issue *has*."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", id="aaaaaaa")
            self.seed(d, "Child", id="bbbbbbb", parent="aaaaaaa")
            epic = json.loads(self.capture(self.t.cmd_show, dir=str(d), id="aaaaaaa",
                                           json=True))
            kid = json.loads(self.capture(self.t.cmd_show, dir=str(d), id="bbbbbbb",
                                          json=True))
            self.assertNotIn("points", epic)
            self.assertIn("points", kid)

    def test_custom_fields_come_through(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha", id="aaaaaaa")
            with redirect_stdout(io.StringIO()):
                self.t.cmd_set(ns(dir=str(d), id="aaaaaaa", priority=None, points=None,
                                  parent=None, title=None, slug=None, spec=None,
                                  status=None, auto=False, review_url=None,
                                  field=["assignee=alice"], unset=None))
            doc = json.loads(self.capture(self.t.cmd_show, dir=str(d), id="aaaaaaa",
                                          json=True))
            self.assertEqual(doc["assignee"], "alice")

    def test_the_human_output_is_untouched(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha", id="aaaaaaa")
            out = self.capture(self.t.cmd_show, dir=str(d), id="aaaaaaa", json=False)
            self.assertIn("--- body ---", out)
            self.assertIn("aaaaaaa", out)
