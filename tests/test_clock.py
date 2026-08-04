"""`TRCK_NOW` fixes the clock, so a command that stamps a date is reproducible.

The conformance fixtures need it: `start` stamps `started`, `done` stamps `closed`, and
`new` stamps `created`, so without a fixed clock any fixture whose expectation includes
`index.jsonl` compares against a value that changes every run.

The alternative was to normalise timestamps in the runner, which would have meant giving
up on asserting *which* date got stamped — and "entering a terminal status stamps
`closed`, leaving it clears it" is real behaviour worth pinning, not noise to filter."""
import json
import os
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker

ENGINE = Path(__file__).resolve().parent.parent / "trck"


class TestFixedClock(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.saved = os.environ.get("TRCK_NOW")

    def tearDown(self):
        if self.saved is None:
            os.environ.pop("TRCK_NOW", None)
        else:
            os.environ["TRCK_NOW"] = self.saved

    def test_unset_uses_the_real_clock(self):
        os.environ.pop("TRCK_NOW", None)
        a, b = self.t.now_utc(), self.t.now_utc()
        self.assertRegex(a, r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
        self.assertLessEqual(a, b)

    def test_it_pins_the_stamp(self):
        os.environ["TRCK_NOW"] = "2026-01-01T00:00:00Z"
        self.assertEqual(self.t.now_utc(), "2026-01-01T00:00:00Z")
        self.assertEqual(self.t.now_utc(), "2026-01-01T00:00:00Z")

    def test_an_offset_is_normalised_to_utc(self):
        """Accepted in any ISO-8601 form so a fixture can be written in whatever the
        author has to hand, but stored in the one canonical shape the engine writes."""
        os.environ["TRCK_NOW"] = "2026-01-01T09:00:00+03:00"
        self.assertEqual(self.t.now_utc(), "2026-01-01T06:00:00Z")

    def test_a_day_only_value_is_refused(self):
        """Day-only dates are a legacy shape the engine no longer writes (see
        scripts/backfill_timestamps.py). Expanding one to midnight would quietly
        reintroduce them through the back door."""
        os.environ["TRCK_NOW"] = "2026-01-01"
        with self.assertRaises(SystemExit):
            self.t.now_utc()

    def test_a_malformed_value_is_refused_not_ignored(self):
        """Silently falling back to the real clock would make a fixture pass locally and
        fail in CI for a reason nothing in the output explains."""
        for bad in ("yesterday", "1735689600", "2026-13-01T00:00:00Z", "x"):
            os.environ["TRCK_NOW"] = bad
            with self.assertRaises(SystemExit, msg=bad):
                self.t.now_utc()

    def test_an_empty_value_means_unset(self):
        os.environ["TRCK_NOW"] = ""
        self.assertRegex(self.t.now_utc(), r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")


class TestTheWholeLifecycleIsReproducible(unittest.TestCase):
    """What the fixtures actually need: the same commands twice, byte-identical
    `index.jsonl`, including every stamped date."""

    def run_engine(self, d, *args, now="2026-03-04T05:06:07Z"):
        env = dict(os.environ, TRCK_NOW=now)
        return subprocess.run([sys.executable, str(ENGINE), "--dir", str(d), *args],
                              capture_output=True, text=True, env=env)

    @unittest.skipUnless(ENGINE.is_file(), "./trck not built")
    def test_two_identical_runs_produce_identical_indexes(self):
        def build(d):
            self.run_engine(d, "new", "Prereq", "--id", "aaaaaaa")
            self.run_engine(d, "new", "Work", "--id", "bbbbbbb", "--depends", "aaaaaaa")
            self.run_engine(d, "start", "aaaaaaa")
            self.run_engine(d, "done", "aaaaaaa")
            return (Path(d) / "index.jsonl").read_text()

        with TemporaryDirectory() as t1, TemporaryDirectory() as t2:
            a = build(make_tracker(t1, {}))
            b = build(make_tracker(t2, {}))
            self.assertEqual(a, b)
            rows = {r["id"]: r for r in
                    (json.loads(l) for l in a.splitlines() if l.strip())}
            self.assertEqual(rows["aaaaaaa"]["created"], "2026-03-04T05:06:07Z")
            self.assertEqual(rows["aaaaaaa"]["started"], "2026-03-04T05:06:07Z")
            self.assertEqual(rows["aaaaaaa"]["closed"], "2026-03-04T05:06:07Z")

    @unittest.skipUnless(ENGINE.is_file(), "./trck not built")
    def test_the_clock_can_advance_between_commands(self):
        """One value per invocation, not one per process lifetime — so a fixture can
        express "created Monday, closed Friday" and assert the difference."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.run_engine(d, "new", "Work", "--id", "aaaaaaa",
                            now="2026-03-02T00:00:00Z")
            self.run_engine(d, "done", "aaaaaaa", now="2026-03-06T00:00:00Z")
            row = json.loads((Path(d) / "index.jsonl").read_text().splitlines()[0])
            self.assertEqual(row["created"], "2026-03-02T00:00:00Z")
            self.assertEqual(row["closed"], "2026-03-06T00:00:00Z")

    @unittest.skipUnless(ENGINE.is_file(), "./trck not built")
    def test_a_malformed_value_fails_the_command_loudly(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            r = self.run_engine(d, "new", "Work", now="not-a-time")
            self.assertNotEqual(r.returncode, 0)
            self.assertIn("TRCK_NOW", r.stderr)
