import json
import os
import shutil
import subprocess
import sys
import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_backfill


class TestToUtc(unittest.TestCase):
    def setUp(self):
        self.b = load_backfill()

    def test_to_utc_converts_offset_to_utc(self):
        self.assertEqual(self.b.to_utc("2026-06-06T12:34:56-03:00"), "2026-06-06T15:34:56Z")

    def test_to_utc_passes_through_utc(self):
        self.assertEqual(self.b.to_utc("2026-06-06T00:00:00+00:00"), "2026-06-06T00:00:00Z")

    def test_to_utc_normalizes_across_day_boundary(self):
        self.assertEqual(self.b.to_utc("2026-06-06T23:30:00-03:00"), "2026-06-07T02:30:00Z")


if __name__ == "__main__":
    unittest.main()
