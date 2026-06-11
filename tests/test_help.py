import argparse
import unittest

from tests.helpers import load_trck


def norm(text):
    """Collapse all whitespace so wrapped help text matches as substrings."""
    return " ".join(text.split())


class TestHelp(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.parser = self.t.build_parser()

    def sub_help(self, name):
        action = next(a for a in self.parser._actions
                      if isinstance(a, argparse._SubParsersAction))
        return norm(action.choices[name].format_help())

    def test_top_level_epilog_teaches_model(self):
        h = norm(self.parser.format_help())
        for token in ("GENERATED", "trck set", "must come first",
                      "TYPICAL FLOW", "trck.json"):
            self.assertIn(token, h)

    def test_top_level_epilog_teaches_recommended_usage(self):
        h = norm(self.parser.format_help())
        for token in ("RECOMMENDED USAGE", "decomposition, not categorization",
                      "Litmus test", "MUST", "SHOULD"):
            self.assertIn(token, h)

    def test_new_help_documents_flags_and_example(self):
        h = self.sub_help("new")
        self.assertIn("comma-separated", h)        # --depends
        self.assertIn("configured kind", h)        # --kind
        self.assertIn("Add CSV export", h)         # epilog example

    def test_list_help_explains_parent_filter(self):
        self.assertIn("children of", self.sub_help("list"))

    def test_dep_help_has_example(self):
        self.assertIn("7 now waits on 5", self.sub_help("dep"))

    def test_set_help_explains_slug_renames_file(self):
        self.assertIn("renames the file", self.sub_help("set"))

    def test_check_description_mentions_before_committing(self):
        self.assertIn("before committing", self.sub_help("check"))

    def test_set_help_documents_custom_fields(self):
        h = self.sub_help("set")
        self.assertIn("--field", h)
        self.assertIn("custom field", h)

    def test_top_level_epilog_shows_custom_field_example(self):
        h = norm(self.parser.format_help())
        self.assertIn("--field assignee=leon", h)
