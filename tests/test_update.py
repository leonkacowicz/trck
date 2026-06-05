import json
import unittest
from tempfile import TemporaryDirectory

from tests.helpers import load_trck


class TestVersionAndFetch(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_parse_version_orders_correctly(self):
        pv = self.t.parse_version
        self.assertEqual(pv("v0.10.1"), (0, 10, 1))
        self.assertTrue(pv("0.10.0") > pv("0.9.9"))
        self.assertTrue(pv("1.0.0") > pv("0.99.0"))

    def test_latest_release_parses_tag_and_body(self):
        payload = json.dumps({"tag_name": "v1.2.3", "body": "notes here"})
        self.t.fetch_url = lambda url, accept=None: payload  # monkeypatch the seam
        tag, body = self.t.latest_release("owner/repo")
        self.assertEqual(tag, "v1.2.3")
        self.assertEqual(body, "notes here")
