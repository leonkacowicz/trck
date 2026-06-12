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


def is_day_only(value) -> bool:
    """True iff value is a bare YYYY-MM-DD string (a legacy date to backfill)."""
    return isinstance(value, str) and DAY_ONLY_RE.match(value) is not None


def rewrite_lines(lines, recovered):
    """Rewrite a list of raw index.jsonl text lines.

    `recovered` maps (id, field) -> git author-date ISO string. For each row,
    each day-only date field is converted to a UTC timestamp if a recovered time
    exists; otherwise it is left untouched and reported as a warning. Lines whose
    fields are all already-timestamped (or non-date) round-trip byte-identically,
    because the input is canonical and dict key order is preserved.

    Returns (new_lines, changes, warnings) where
      changes  = list of (id, field, old, new)
      warnings = list of (id, field, old).
    """
    new_lines, changes, warnings = [], [], []
    for line in lines:
        if not line.strip():
            new_lines.append(line)
            continue
        row = json.loads(line)
        iid = row.get("id")
        for f in DATE_FIELDS:
            v = row.get(f)
            if not is_day_only(v):
                continue
            key = (iid, f)
            if key in recovered:
                new = to_utc(recovered[key])
                row[f] = new
                changes.append((iid, f, v, new))
            else:
                warnings.append((iid, f, v))
        new_lines.append(json.dumps(row, ensure_ascii=False))
    return new_lines, changes, warnings


def reduce_transitions(snapshots):
    """Fold an oldest->newest sequence of (author_iso, rows) into
    {(id, field): author_iso}.

    `rows` is the list of issue dicts from index.jsonl at that commit. A field is
    "recovered" at the author date of every commit where its value changes to a
    new non-null value; later transitions overwrite earlier ones, so the final
    value is the author date of the LAST commit that set the field to the value
    it has at the end of history. Clearing a field to None records nothing and
    does not erase a prior recovered time.
    """
    recovered, prev = {}, {}
    for author_iso, rows in snapshots:
        for row in rows:
            iid = row.get("id")
            if not isinstance(iid, int) or isinstance(iid, bool):
                continue
            pv = prev.setdefault(iid, {})
            for f in DATE_FIELDS:
                cur = row.get(f)
                if cur is not None and cur != pv.get(f):
                    recovered[(iid, f)] = author_iso
                pv[f] = cur
    return recovered


def _git(root, *args):
    """Run a git command in `root`; returns the CompletedProcess (text mode)."""
    try:
        return subprocess.run(["git", "-C", str(root), *args],
                              capture_output=True, text=True)
    except FileNotFoundError:
        sys.exit("error: git not found on PATH")


def resolve_index(tracker_dir):
    """Return (git_root, index_rel, index_path) for a tracker dir, or exit with a
    clear error if git is missing, the dir is not in a repo, or there is no
    index.jsonl."""
    d = Path(tracker_dir)
    index_path = d / "index.jsonl"
    if not index_path.is_file():
        sys.exit(f"error: {index_path} not found")
    r = _git(d, "rev-parse", "--show-toplevel")
    if r.returncode != 0:
        sys.exit(f"error: {d} is not inside a git repository")
    root = Path(r.stdout.strip())
    try:
        index_rel = index_path.resolve().relative_to(root.resolve())
    except ValueError:
        sys.exit(f"error: {index_path} is not under git root {root}")
    return root, str(index_rel), index_path


def list_history(root, index_rel):
    """Commits touching the index, oldest->newest, as (sha, author_iso) pairs."""
    r = _git(root, "log", "--reverse", "--format=%H%x09%aI", "--", index_rel)
    if r.returncode != 0:
        sys.exit(f"error: git log failed: {r.stderr.strip()}")
    out = []
    for line in r.stdout.splitlines():
        if not line.strip():
            continue
        sha, _, author_iso = line.partition("\t")
        # Normalize the trailing "Z" that some git versions emit for UTC
        if author_iso.endswith("Z"):
            author_iso = author_iso[:-1] + "+00:00"
        out.append((sha, author_iso))
    return out


def read_index_at(root, index_rel, sha):
    """Parse the index blob at a commit into a list of row dicts, or None if the
    blob is missing (path didn't exist there) or unparseable."""
    r = _git(root, "show", f"{sha}:{index_rel}")
    if r.returncode != 0:
        return None
    rows = []
    for line in r.stdout.splitlines():
        if not line.strip():
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError:
            return None
    return rows


def recover_times(root, index_rel):
    """Walk the index's git history and recover {(id, field): author_iso}."""
    snapshots = []
    for sha, author_iso in list_history(root, index_rel):
        rows = read_index_at(root, index_rel, sha)
        if rows is None:
            continue
        snapshots.append((author_iso, rows))
    return reduce_transitions(snapshots)
