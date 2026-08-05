"""What is left of the changelog suite after the conformance conversion (#fpcvhp8).

Everything the verb *prints* — ordering, nesting, the shipped/not-shipped rule, the cutoff
semantics, counts and the error path — lives in the `changelog-*` conformance fixtures, which
run against both engines. What stays here is the part a fixture cannot reach: the `parse_since`
helper called directly, and the argparse wiring.
"""
import unittest

from tests.helpers import load_trck


class TestParseSince(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_accepts_bare_date(self):
        self.assertEqual(self.t.parse_since("2026-06-10"), "2026-06-10")

    def test_accepts_full_timestamp(self):
        self.assertEqual(self.t.parse_since("2026-06-10T14:00:00Z"), "2026-06-10T14:00:00Z")

    def test_rejects_garbage(self):
        for bad in ("june", "2026/06/10", "2026-6-10", "2026-06-10T14:00Z", ""):
            with self.subTest(bad=bad), self.assertRaises(SystemExit):
                self.t.parse_since(bad)


class TestCmdChangelog(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_changelog_is_a_registered_subcommand(self):
        p = self.t.build_parser()
        args = p.parse_args(["changelog", "--since", "2026-06-10"])
        self.assertIs(args.func, self.t.cmd_changelog)
        self.assertEqual(args.since, "2026-06-10")


if __name__ == "__main__":
    unittest.main()
