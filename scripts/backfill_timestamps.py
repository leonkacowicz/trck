#!/usr/bin/env python3
"""Backfill day-only created/started/closed dates in a trck index.jsonl with
full UTC timestamps recovered from git history (the author date of the commit
that set each field to its current value).

Standalone, standard-library only. Does NOT import the trck engine, so it can be
run against any repo that uses trck:

    python3 scripts/backfill_timestamps.py [TRACKER_DIR] [--dry-run]

TRACKER_DIR defaults to "issues". Rewrites index.jsonl in place (unless
--dry-run). Idempotent: values already in timestamp form are left untouched.
"""
import argparse
import json
import re
import subprocess
import sys
from datetime import datetime, timezone
from pathlib import Path

DATE_FIELDS = ("created", "started", "closed")
DAY_ONLY_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


def to_utc(author_iso: str) -> str:
    """Convert a git author date (ISO 8601 with offset) to the engine's
    canonical UTC stamp: second-precision, 'Z'-suffixed, no microseconds."""
    return datetime.fromisoformat(author_iso).astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
