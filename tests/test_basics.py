import unittest
from tests.helpers import load_trck


class TestSkeleton(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_has_semantic_version(self):
        import re
        self.assertRegex(self.t.__version__, r"^\d+\.\d+\.\d+$")

    def test_default_update_repo(self):
        self.assertEqual(self.t.DEFAULT_UPDATE_REPO, "leonkacowicz/trck")

    def test_slugify(self):
        self.assertEqual(self.t.slugify("Hello, World! 2"), "hello-world-2")
        self.assertEqual(self.t.slugify("--Trim--"), "trim")

    def test_today_isoformat(self):
        import re
        self.assertRegex(self.t.today(), r"^\d{4}-\d{2}-\d{2}$")
