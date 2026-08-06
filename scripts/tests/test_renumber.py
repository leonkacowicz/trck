"""Tests for scripts/renumber.py — the one-shot converter for trackers that still
carry trck's first-iteration integer ids.

Kept out of `tests/` for the same reason the script is kept out of the engine: the
conversion is not part of the tool any more. The engine's side of this — refusing a
tracker that still has integer ids — is engine behaviour and is tested there.

The one claim that spans both, "the engine accepts what the script produces", is made
here by running `./trck check` as a subprocess rather than importing the engine, so
this suite stays free of it.

    python3 -m unittest discover -s scripts/tests
"""
import json
import os
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

REPO_ROOT = Path(__file__).resolve().parent.parent.parent
SCRIPT_PATH = REPO_ROOT / "scripts" / "renumber.py"
# The built binary: this asserts that the engine accepts what the converter produced, and an
# installed trck would answer for a version that is not the one under change.
ENGINE_PATH = Path(os.environ.get("TRCK_BIN") or REPO_ROOT / "target" / "release" / "trck")


def load_renumber():
    """Import the script as a fresh module object."""
    import importlib.machinery
    import importlib.util
    loader = importlib.machinery.SourceFileLoader("renumber", str(SCRIPT_PATH))
    spec = importlib.util.spec_from_file_location("renumber", SCRIPT_PATH, loader=loader)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["renumber"] = mod
    spec.loader.exec_module(mod)
    return mod


def legacy_tracker(tmp):
    """A tracker as the old engine would have written one: integer ids in the index,
    zero-padded filenames, an integer parent link, and a `#NN` body reference."""
    d = Path(tmp) / "issues"
    (d / "items").mkdir(parents=True, exist_ok=True)
    (d / "trck.json").write_text("{}")
    rows = [
        {"id": "1", "slug": "epic", "title": "Epic", "status": "backlog",
         "priority": "high", "created": "2026-01-01T00:00:00Z"},
        {"id": "2", "slug": "task", "title": "Task", "status": "backlog",
         "priority": "high", "parent": "1", "created": "2026-01-01T00:00:00Z"},
    ]
    (d / "index.jsonl").write_text("".join(json.dumps(r) + "\n" for r in rows))
    (d / "items" / "001-epic.md").write_text("# Epic\n")
    (d / "items" / "002-task.md").write_text("# Task\n\nPart of #1.\n")
    return d


class TestRenumber(unittest.TestCase):
    def setUp(self):
        self.r = load_renumber()

    def run_script(self, d, *extra):
        return subprocess.run([sys.executable, str(SCRIPT_PATH), str(d), *extra],
                              capture_output=True, text=True)

    def rows(self, d):
        return [json.loads(l) for l in
                (Path(d) / "index.jsonl").read_text().splitlines() if l.strip()]

    def test_is_legacy_uses_length_not_digit_ness(self):
        """`2345678` is a legal random id — the alphabet contains digits — so digits
        alone cannot be the discriminator."""
        self.assertTrue(self.r.is_legacy(24))
        self.assertTrue(self.r.is_legacy("24"))
        self.assertTrue(self.r.is_legacy("024"))
        self.assertFalse(self.r.is_legacy("2345678"))
        self.assertFalse(self.r.is_legacy("k3m9x2a"))

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
            res = self.run_script(d)
            self.assertEqual(res.returncode, 0, res.stderr)

            new = {r["title"]: r["id"] for r in self.rows(d)}
            for iid in new.values():
                self.assertEqual(len(iid), self.r.ID_LEN)
                self.assertTrue(set(iid) <= set(self.r.ID_ALPHABET))
            task = next(r for r in self.rows(d) if r["title"] == "Task")
            self.assertEqual(task["parent"], new["Epic"])
            items = {p.name: p.read_text() for p in (Path(d) / "items").glob("*.md")}
            self.assertIn(f"{new['Epic']}-epic.md", items)
            self.assertIn(f"#{new['Epic']}", items[f"{new['Task']}-task.md"])

    def test_it_writes_the_map_because_commit_messages_are_not_rewritable(self):
        """The engine used to store each issue's old number and resolve `trck show 24`
        through it. It does not any more, so this file is the only way left to read a
        `#24` in history."""
        with TemporaryDirectory() as tmp:
            d = legacy_tracker(tmp)
            self.run_script(d)
            m = json.loads((Path(d) / "legacy-ids.json").read_text())
            self.assertEqual(sorted(m), ["1", "2"])
            self.assertEqual(sorted(m.values()),
                             sorted(r["id"] for r in self.rows(d)))

    def test_a_second_run_is_a_no_op(self):
        with TemporaryDirectory() as tmp:
            d = legacy_tracker(tmp)
            self.run_script(d)
            before = (Path(d) / "index.jsonl").read_text()
            r = self.run_script(d)
            self.assertIn("nothing to convert", r.stdout)
            self.assertEqual((Path(d) / "index.jsonl").read_text(), before)

    @unittest.skipUnless(ENGINE_PATH.is_file(), "engine not built (cargo build --release)")
    def test_the_engine_accepts_what_the_script_produces(self):
        """The claim that spans both sides, made by subprocess so this suite keeps its
        independence from the engine: a legacy tracker is refused, and a converted one is
        clean.

        Only the refusal's *existence* is asserted, not its wording. The engine used to
        recognise integer ids by name and point at this script; it no longer does, and that
        was decided rather than lost (`#t4azhkq`) — a tracker this old is unreadable, and
        saying so imprecisely beats promising a conversion the engine does not implement."""
        with TemporaryDirectory() as tmp:
            d = legacy_tracker(tmp)
            before = subprocess.run([str(ENGINE_PATH), "--dir", str(d), "check"],
                                    capture_output=True, text=True)
            self.assertNotEqual(before.returncode, 0, "a legacy tracker was accepted")

            self.run_script(d)
            after = subprocess.run([str(ENGINE_PATH), "--dir", str(d), "check"],
                                   capture_output=True, text=True)
            self.assertEqual(after.returncode, 0, after.stderr)
            self.assertIn("OK", after.stdout)
