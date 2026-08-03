"""index.jsonl ids must be unique.

A duplicate id makes the in-memory model ambiguous: `Graph.by_id` keeps one row while
both are counted and listed, and `resolve_ref` picks `exact[0]`, so a mutating verb
operates on an arbitrary one of them. It is a structural defect in the file, not a
recoverable inconsistency, so `load_index` refuses it the same way it refuses malformed
JSON — but it collects every duplicated id first, so one run reports the whole problem.

This is the state `merge=union` on index.jsonl produces when two branches both mutate
one issue (see #ey2aruc).
"""
import io
import json
import unittest
from contextlib import redirect_stdout, redirect_stderr
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


def row(iid, status="backlog", slug="alpha"):
    return {"id": iid, "slug": slug, "title": slug.title(),
            "status": status, "priority": "high"}


class TestDuplicateIds(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def tracker(self, tmp, rows):
        d = make_tracker(tmp, {})
        (d / "index.jsonl").write_text(
            "".join(json.dumps(r) + "\n" for r in rows))
        return d

    def load(self, d):
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return self.t.load_index(ctx)

    def die_message(self, d):
        """Run load_index expecting SystemExit; return what it printed to stderr."""
        buf = io.StringIO()
        with self.assertRaises(SystemExit):
            with redirect_stderr(buf):
                self.load(d)
        return buf.getvalue()

    def test_clean_index_still_loads(self):
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, [row("abc1234"), row("bcd2345", slug="beta")])
            self.assertEqual([r.id for r in self.load(d)], ["abc1234", "bcd2345"])

    def test_duplicate_id_is_refused(self):
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, [row("abc1234", "ongoing"),
                                   row("abc1234", "done")])
            self.assertIn("abc1234", self.die_message(d))

    def test_message_names_the_conflicting_statuses(self):
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, [row("abc1234", "ongoing"),
                                   row("abc1234", "done")])
            msg = self.die_message(d)
            self.assertIn("ongoing", msg)
            self.assertIn("done", msg)

    def test_three_rows_one_id_report_that_id_once(self):
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, [row("abc1234", "backlog"),
                                   row("abc1234", "ongoing"),
                                   row("abc1234", "done")])
            msg = self.die_message(d)
            self.assertEqual(msg.count("abc1234"), 1, msg)

    def test_every_duplicated_id_is_reported_in_one_run(self):
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, [row("abc1234", "ongoing"),
                                   row("abc1234", "done"),
                                   row("bcd2345", "backlog", slug="beta"),
                                   row("bcd2345", "done", slug="beta")])
            msg = self.die_message(d)
            self.assertIn("abc1234", msg)
            self.assertIn("bcd2345", msg)

    def test_check_exits_nonzero_on_a_duplicate(self):
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, [row("abc1234", "ongoing"),
                                   row("abc1234", "done")])
            (d / "items").mkdir()
            (d / "items" / "abc1234-alpha.md").write_text("# Alpha\n")
            with self.assertRaises(SystemExit):
                with redirect_stdout(io.StringIO()), redirect_stderr(io.StringIO()):
                    self.t.cmd_check(ns(dir=str(d)))

    def test_distinct_ids_sharing_a_slug_are_fine(self):
        """Only the id must be unique — two issues may carry the same slug."""
        with TemporaryDirectory() as tmp:
            d = self.tracker(tmp, [row("abc1234", slug="alpha"),
                                   row("bcd2345", slug="alpha")])
            self.assertEqual(len(self.load(d)), 2)
