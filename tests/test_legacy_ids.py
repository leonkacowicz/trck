"""Integer ids are gone: the engine no longer reads, writes or resolves them, and
refuses a tracker that still has them.

Only the engine's side is here. The conversion itself lives in `scripts/renumber.py`
and is tested from `scripts/tests/` — it is not part of the executable any more, so it
does not belong in the suite that guards the executable.

They were trck's first iteration, replaced because two branches running `trck new`
minted the same number. What outlived them was a migration path that cost more than
the verb: a `legacy_id` field and its resolution tier, a zero-padded filename
convention, an int-accepting id validator, and a rule in prefix generation reserving
all-digit draws to keep `all-digit ⇔ legacy` sound."""
import json
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


def legacy_tracker(tmp):
    """A tracker as the old engine would have written one: integer ids in the index,
    zero-padded filenames, an integer `parent`, and a `#NN` body reference."""
    d = make_tracker(tmp, {})
    items = Path(d) / "items"
    items.mkdir(exist_ok=True)
    rows = [
        {"id": "1", "slug": "epic", "title": "Epic", "status": "backlog",
         "priority": "high", "created": "2026-01-01T00:00:00Z"},
        {"id": "2", "slug": "task", "title": "Task", "status": "backlog",
         "priority": "high", "parent": "1", "created": "2026-01-01T00:00:00Z"},
    ]
    (Path(d) / "index.jsonl").write_text(
        "".join(json.dumps(r) + "\n" for r in rows))
    (items / "001-epic.md").write_text("# Epic\n")
    (items / "002-task.md").write_text("# Task\n\nPart of #1.\n")
    return d


class TestTheEngineRefuses(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_a_legacy_tracker_is_refused_by_name(self):
        """The fallout otherwise is obscure: `filename` no longer zero-pads, so every
        issue reports as both missing on disk and misnamed, and nothing in that output
        says why."""
        with TemporaryDirectory() as tmp:
            d = legacy_tracker(tmp)
            with self.assertRaises(SystemExit):
                self.t.build_ctx_or_die(ns(dir=str(d)))

    def test_the_refusal_names_the_converter(self):
        with TemporaryDirectory() as tmp:
            d = legacy_tracker(tmp)
            names = self.t.detect_legacy_ids(Path(d))
            self.assertEqual(names, ["001-epic.md", "002-task.md"])

    def test_an_all_digit_random_id_is_not_mistaken_for_a_legacy_one(self):
        """`2345678` is a legal draw — the alphabet contains digits. Length is what
        separates it from `024`, not digit-ness."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            items = Path(d) / "items"; items.mkdir(exist_ok=True)
            (items / "2345678-ok.md").write_text("# Ok\n")
            self.assertEqual(self.t.detect_legacy_ids(Path(d)), [])

    def test_an_integer_id_in_the_index_is_a_parse_error(self):
        with self.assertRaises(ValueError) as cm:
            self.t.Issue.from_dict({"id": 24, "slug": "s", "title": "T",
                                    "status": "backlog", "priority": "high"})
        self.assertIn("must be a string id", str(cm.exception))

    def test_renumber_is_not_a_verb_any_more(self):
        self.assertFalse(hasattr(self.t, "cmd_renumber"))
        with self.assertRaises(SystemExit):   # argparse rejects the unknown subcommand
            self.t.build_parser().parse_args(["repo", "renumber"])
