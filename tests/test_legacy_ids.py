"""Integer ids are gone — the engine refuses a tracker that still has them, and the
conversion lives in a standalone script rather than in the engine.

They were trck's first iteration, replaced because two branches running `trck new`
minted the same number. What outlived them was a migration path that cost more than
the verb: a `legacy_id` field and its resolution tier, a zero-padded filename
convention, an int-accepting id validator, and a rule in prefix generation reserving
all-digit draws to keep `all-digit ⇔ legacy` sound."""
import json
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns

SCRIPT = Path(__file__).resolve().parent.parent / "scripts" / "renumber.py"


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


class TestTheConverter(unittest.TestCase):
    """It lives in scripts/, does not import the engine, and therefore runs against a
    tracker the installed engine already refuses."""

    def setUp(self):
        self.t = load_trck()

    def run_script(self, d, *extra):
        return subprocess.run([sys.executable, str(SCRIPT), str(d), *extra],
                              capture_output=True, text=True)

    def test_dry_run_writes_nothing(self):
        with TemporaryDirectory() as tmp:
            d = legacy_tracker(tmp)
            before = (Path(d) / "index.jsonl").read_text()
            r = self.run_script(d, "--dry-run")
            self.assertEqual(r.returncode, 0, r.stderr)
            self.assertIn("#1 -> #", r.stdout)
            self.assertEqual((Path(d) / "index.jsonl").read_text(), before)
            self.assertFalse((Path(d) / "legacy-ids.json").exists())

    def test_it_converts_ids_links_filenames_and_body_references(self):
        with TemporaryDirectory() as tmp:
            d = legacy_tracker(tmp)
            r = self.run_script(d)
            self.assertEqual(r.returncode, 0, r.stderr)

            rows = [json.loads(l) for l in
                    (Path(d) / "index.jsonl").read_text().splitlines() if l.strip()]
            new = {r["title"]: r["id"] for r in rows}
            for iid in new.values():
                self.assertEqual(len(iid), self.t.ID_LEN)
                self.assertTrue(set(iid) <= set(self.t.ID_ALPHABET))
            # the parent link followed the rename
            task = next(r for r in rows if r["title"] == "Task")
            self.assertEqual(task["parent"], new["Epic"])
            # so did the filenames, and the `#1` in the body
            items = {p.name: p.read_text() for p in (Path(d) / "items").glob("*.md")}
            self.assertIn(f"{new['Epic']}-epic.md", items)
            self.assertIn(f"#{new['Epic']}", items[f"{new['Task']}-task.md"])

    def test_it_writes_the_map_because_commit_messages_are_not_rewritable(self):
        """The engine used to resolve `trck show 24` through a stored `legacy_id`.
        It does not any more, so this file is the only way to read a `#24` in
        history."""
        with TemporaryDirectory() as tmp:
            d = legacy_tracker(tmp)
            self.run_script(d)
            m = json.loads((Path(d) / "legacy-ids.json").read_text())
            self.assertEqual(sorted(m), ["1", "2"])

    def test_the_converted_tracker_passes_check(self):
        with TemporaryDirectory() as tmp:
            d = legacy_tracker(tmp)
            self.run_script(d)
            self.assertEqual(self.t.detect_legacy_ids(Path(d)), [])
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            self.assertEqual(self.t.validate(ctx, self.t.load_index(ctx))[0], [])

    def test_a_second_run_is_a_no_op(self):
        with TemporaryDirectory() as tmp:
            d = legacy_tracker(tmp)
            self.run_script(d)
            before = (Path(d) / "index.jsonl").read_text()
            r = self.run_script(d)
            self.assertIn("nothing to convert", r.stdout)
            self.assertEqual((Path(d) / "index.jsonl").read_text(), before)
