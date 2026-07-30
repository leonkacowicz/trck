"""The `trck repo` group: tracker-maintenance verbs live under it, not at the root.

`init` and `update` deliberately stay at the root — they act on your *setup*, not on
a tracker. `init` in particular runs before a tracker exists. The move is a clean
break: the old flat spellings are gone, not aliased.
"""
import argparse
import io
import unittest
from contextlib import redirect_stderr

from tests.helpers import load_trck

GROUPED = ["normalize", "renumber", "install-hook"]
ROOT_ONLY = ["init", "update"]


class TestRepoGroup(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.parser = self.t.build_parser()

    def root_choices(self):
        action = next(a for a in self.parser._actions
                      if isinstance(a, argparse._SubParsersAction))
        return action.choices

    def repo_choices(self):
        repo = self.root_choices()["repo"]
        action = next(a for a in repo._actions
                      if isinstance(a, argparse._SubParsersAction))
        return action.choices

    def test_repo_group_exists_at_the_root(self):
        self.assertIn("repo", self.root_choices())

    def test_maintenance_verbs_live_under_repo(self):
        choices = self.repo_choices()
        for verb in GROUPED:
            self.assertIn(verb, choices, f"{verb} should be under `trck repo`")

    def test_maintenance_verbs_are_gone_from_the_root(self):
        choices = self.root_choices()
        for verb in GROUPED:
            self.assertNotIn(verb, choices,
                             f"{verb} should no longer be a root verb")

    def test_init_and_update_stay_at_the_root(self):
        choices = self.root_choices()
        for verb in ROOT_ONLY:
            self.assertIn(verb, choices)

    def test_old_flat_spelling_is_rejected(self):
        """A clean break: no hidden aliases. argparse rejects the old form."""
        for verb in GROUPED:
            with self.subTest(verb=verb):
                with self.assertRaises(SystemExit):
                    with redirect_stderr(io.StringIO()):
                        self.parser.parse_args([verb])

    def test_grouped_verbs_dispatch_to_their_handlers(self):
        expected = {
            "normalize": self.t.cmd_normalize,
            "renumber": self.t.cmd_renumber,
            "install-hook": self.t.cmd_install_hook,
        }
        for verb, func in expected.items():
            with self.subTest(verb=verb):
                args = self.parser.parse_args(["repo", verb])
                self.assertIs(args.func, func)

    def test_repo_without_a_subverb_errors(self):
        with self.assertRaises(SystemExit):
            with redirect_stderr(io.StringIO()):
                self.parser.parse_args(["repo"])

    def test_dir_still_resolves_before_the_group(self):
        """--dir is a top-level flag; it must keep working with a nested verb."""
        args = self.parser.parse_args(["--dir", "/tmp/x", "repo", "normalize"])
        self.assertEqual(args.dir, "/tmp/x")

    def test_repo_help_lists_the_grouped_verbs(self):
        text = " ".join(self.root_choices()["repo"].format_help().split())
        for verb in GROUPED:
            self.assertIn(verb, text)

    def test_root_help_lists_the_group_not_its_members(self):
        text = " ".join(self.parser.format_help().split())
        self.assertIn("repo", text)
        self.assertIn("init", text)
        self.assertNotIn("rewrite index.jsonl in canonical slim form", text)
