"""What is left of the `--json` suite after the conformance conversion (#t84am5s).

Every *shape* — the nested and flat lists, both `deps` cones, `show`'s folded-in body,
`ready`'s rank order and its demand fields, the filters, the empty results and the two
error paths — now lives in the `*-json-*` conformance fixtures, which run against both
engines. What stays here is the seam itself: `emit_json` called directly, at the one
property no fixture can express, since a fixture only ever sees a *single* invocation's
stdout and so cannot tell "one document" from "the first of two".
"""
import io
import json
import unittest
from contextlib import redirect_stdout

from tests.helpers import load_trck


class TestEmitJson(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

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


if __name__ == "__main__":
    unittest.main()
