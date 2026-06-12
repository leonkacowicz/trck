# Git-history Timestamp Backfill Script Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A standalone, stdlib-only script that rewrites day-only `created`/`started`/`closed` values in a trck `index.jsonl` into full UTC timestamps recovered from the author date of the git commit that set each field to its current value.

**Architecture:** Pure helpers (`to_utc`, `is_day_only`, `rewrite_lines`, `reduce_transitions`) do the conversion and the byte-faithful line rewrite; a thin git layer (`resolve_index`, `list_history`, `read_index_at`, `recover_times`) feeds them history snapshots; `main()` wires it together, writes in place (unless `--dry-run`), and prints a report. The script never imports the trck engine, so it is portable across repos.

**Tech Stack:** Python 3 standard library only (`subprocess`, `json`, `re`, `datetime`, `argparse`, `pathlib`, `sys`). Tests under `tests/` run via `python3 -m unittest discover -s tests`; they import the script through a `SourceFileLoader` helper and shell out to real `git` for integration.

**Spec:** `docs/specs/2026-06-12-timestamp-backfill-script-design.md`

---

## File Structure

- **`scripts/backfill_timestamps.py`** (new) — the whole script. One file: constants → pure helpers → git layer → `main`. Underscore name so tests can import it.
- **`tests/helpers.py`** (modify) — add a `load_backfill()` loader mirroring `load_trck()`.
- **`tests/test_backfill.py`** (new) — unit tests for the pure helpers and git-backed tests (temp repo) for the git layer and `main()`.

The engine `./trck` is **not** touched by this sub-project.

Reference — trck's canonical index serialization (so the rewrite stays byte-faithful) is `json.dumps(row, ensure_ascii=False)` with default separators, keys in insertion order (`trck:420-422`). Re-dumping a parsed canonical line reproduces it exactly.

---

### Task 1: Script scaffold + `to_utc()` + test loader

**Files:**
- Create: `scripts/backfill_timestamps.py`
- Modify: `tests/helpers.py`
- Test: `tests/test_backfill.py` (new)

- [ ] **Step 1: Add the loader to `tests/helpers.py`**

Append to `tests/helpers.py` (it already defines `REPO_ROOT`):

```python
SCRIPT_PATH = REPO_ROOT / "scripts" / "backfill_timestamps.py"


def load_backfill():
    """Import scripts/backfill_timestamps.py as a fresh module object."""
    import importlib.machinery
    import importlib.util
    import sys
    loader = importlib.machinery.SourceFileLoader("backfill_timestamps", str(SCRIPT_PATH))
    spec = importlib.util.spec_from_file_location("backfill_timestamps", SCRIPT_PATH, loader=loader)
    mod = importlib.util.module_from_spec(spec)
    sys.modules["backfill_timestamps"] = mod
    spec.loader.exec_module(mod)
    return mod
```

- [ ] **Step 2: Write the failing test**

Create `tests/test_backfill.py` (all imports the later tasks need are included up front, so subsequent tasks only append classes):

```python
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
```

- [ ] **Step 3: Run it to verify it fails**

Run: `python3 -m unittest tests.test_backfill -v`
Expected: FAIL — `FileNotFoundError`/`ImportError` (the script file doesn't exist yet), or `AttributeError: module 'backfill_timestamps' has no attribute 'to_utc'`.

- [ ] **Step 4: Create the script scaffold with `to_utc()`**

Create `scripts/backfill_timestamps.py`:

```python
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
```

- [ ] **Step 5: Run the test to verify it passes**

Run: `python3 -m unittest tests.test_backfill -v`
Expected: PASS (3 tests).

- [ ] **Step 6: Commit**

```bash
git add scripts/backfill_timestamps.py tests/helpers.py tests/test_backfill.py
git commit -m "backfill: script scaffold and to_utc() git-date conversion"
```

---

### Task 2: `is_day_only()` + `rewrite_lines()`

**Files:**
- Modify: `scripts/backfill_timestamps.py`
- Test: `tests/test_backfill.py`

- [ ] **Step 1: Write the failing tests**

Append to `tests/test_backfill.py` (imports were all added in Task 1):

```python
class TestRewriteLines(unittest.TestCase):
    def setUp(self):
        self.b = load_backfill()

    def canonical(self, **row):
        return json.dumps(row, ensure_ascii=False)

    def test_is_day_only(self):
        self.assertTrue(self.b.is_day_only("2026-06-05"))
        self.assertFalse(self.b.is_day_only("2026-06-05T00:00:00Z"))
        self.assertFalse(self.b.is_day_only(None))
        self.assertFalse(self.b.is_day_only(""))

    def test_rewrite_replaces_day_only_and_leaves_others_byte_identical(self):
        recovered = {
            (1, "created"): "2026-06-05T09:00:00+00:00",
            (1, "closed"): "2026-06-06T12:00:00-03:00",
        }
        line1 = self.canonical(id=1, slug="x", title="X", kind="task",
                               status="done", priority="medium",
                               created="2026-06-05", closed="2026-06-06")
        line2 = self.canonical(id=2, slug="y", title="Y", kind="task",
                               status="backlog", priority="low",
                               created="2026-06-05T08:00:00Z")
        new_lines, changes, warnings = self.b.rewrite_lines([line1, line2], recovered)
        # issue 2 already timestamped -> untouched and byte-identical
        self.assertEqual(new_lines[1], line2)
        # issue 1's day-only fields converted to UTC
        row1 = json.loads(new_lines[0])
        self.assertEqual(row1["created"], "2026-06-05T09:00:00Z")
        self.assertEqual(row1["closed"], "2026-06-06T15:00:00Z")
        self.assertEqual(set(changes), {
            (1, "created", "2026-06-05", "2026-06-05T09:00:00Z"),
            (1, "closed", "2026-06-06", "2026-06-06T15:00:00Z"),
        })
        self.assertEqual(warnings, [])

    def test_rewrite_warns_when_no_history_and_leaves_value(self):
        line = self.canonical(id=3, slug="z", title="Z", kind="task",
                              status="done", priority="medium", created="2026-06-05")
        new_lines, changes, warnings = self.b.rewrite_lines([line], {})
        self.assertEqual(new_lines[0], line)  # unchanged
        self.assertEqual(changes, [])
        self.assertEqual(warnings, [(3, "created", "2026-06-05")])

    def test_rewrite_preserves_blank_lines(self):
        new_lines, changes, warnings = self.b.rewrite_lines(["", "  "], {})
        self.assertEqual(new_lines, ["", "  "])
        self.assertEqual(changes, [])
        self.assertEqual(warnings, [])
```

- [ ] **Step 2: Run to verify they fail**

Run: `python3 -m unittest tests.test_backfill.TestRewriteLines -v`
Expected: FAIL — `AttributeError: module 'backfill_timestamps' has no attribute 'is_day_only'`.

- [ ] **Step 3: Implement `is_day_only` and `rewrite_lines`**

In `scripts/backfill_timestamps.py`, add after `to_utc`:

```python
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `python3 -m unittest tests.test_backfill.TestRewriteLines -v`
Expected: PASS (5 tests).

- [ ] **Step 5: Commit**

```bash
git add scripts/backfill_timestamps.py tests/test_backfill.py
git commit -m "backfill: day-only guard and byte-faithful line rewrite"
```

---

### Task 3: `reduce_transitions()`

**Files:**
- Modify: `scripts/backfill_timestamps.py`
- Test: `tests/test_backfill.py`

- [ ] **Step 1: Write the failing tests**

Append to `tests/test_backfill.py`:

```python
class TestReduceTransitions(unittest.TestCase):
    def setUp(self):
        self.b = load_backfill()

    def test_first_appearance_sets_created(self):
        snaps = [("2026-06-01T10:00:00+00:00", [{"id": 1, "created": "2026-06-01"}])]
        rec = self.b.reduce_transitions(snaps)
        self.assertEqual(rec[(1, "created")], "2026-06-01T10:00:00+00:00")

    def test_last_close_wins_after_reopen(self):
        snaps = [
            ("2026-06-01T10:00:00+00:00", [{"id": 1, "created": "2026-06-01"}]),
            ("2026-06-02T10:00:00+00:00", [{"id": 1, "created": "2026-06-01", "closed": "2026-06-02"}]),
            ("2026-06-03T10:00:00+00:00", [{"id": 1, "created": "2026-06-01"}]),               # reopened: closed cleared
            ("2026-06-04T10:00:00+00:00", [{"id": 1, "created": "2026-06-01", "closed": "2026-06-04"}]),  # reclosed
        ]
        rec = self.b.reduce_transitions(snaps)
        self.assertEqual(rec[(1, "closed")], "2026-06-04T10:00:00+00:00")
        self.assertEqual(rec[(1, "created")], "2026-06-01T10:00:00+00:00")

    def test_clear_to_none_keeps_last_set_time(self):
        snaps = [
            ("2026-06-02T10:00:00+00:00", [{"id": 1, "closed": "2026-06-02"}]),
            ("2026-06-03T10:00:00+00:00", [{"id": 1}]),  # cleared
        ]
        rec = self.b.reduce_transitions(snaps)
        self.assertEqual(rec[(1, "closed")], "2026-06-02T10:00:00+00:00")

    def test_non_integer_id_rows_are_ignored(self):
        snaps = [("2026-06-01T10:00:00+00:00", [{"slug": "noid", "created": "2026-06-01"}])]
        rec = self.b.reduce_transitions(snaps)
        self.assertEqual(rec, {})
```

- [ ] **Step 2: Run to verify they fail**

Run: `python3 -m unittest tests.test_backfill.TestReduceTransitions -v`
Expected: FAIL — `AttributeError: module 'backfill_timestamps' has no attribute 'reduce_transitions'`.

- [ ] **Step 3: Implement `reduce_transitions`**

In `scripts/backfill_timestamps.py`, add after `rewrite_lines`:

```python
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `python3 -m unittest tests.test_backfill.TestReduceTransitions -v`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add scripts/backfill_timestamps.py tests/test_backfill.py
git commit -m "backfill: transition reducer over git history snapshots"
```

---

### Task 4: Git layer (`resolve_index`, `list_history`, `read_index_at`, `recover_times`)

**Files:**
- Modify: `scripts/backfill_timestamps.py`
- Test: `tests/test_backfill.py`

- [ ] **Step 1: Write the failing tests**

Append to `tests/test_backfill.py` (imports were all added in Task 1). Add these module-level helpers and the test class:

```python
def _git(cwd, *args):
    env = dict(
        os.environ,
        GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@e",
        GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@e",
    )
    subprocess.run(["git", "-C", str(cwd), *args], check=True,
                   capture_output=True, text=True, env=env)


def _commit_index(repo, content, author_iso):
    """Write issues/index.jsonl and commit it with a fixed author/committer date."""
    issues = Path(repo) / "issues"
    issues.mkdir(exist_ok=True)
    (issues / "index.jsonl").write_text(content)
    env = dict(
        os.environ,
        GIT_AUTHOR_NAME="t", GIT_AUTHOR_EMAIL="t@e",
        GIT_COMMITTER_NAME="t", GIT_COMMITTER_EMAIL="t@e",
        GIT_AUTHOR_DATE=author_iso, GIT_COMMITTER_DATE=author_iso,
    )
    subprocess.run(["git", "-C", str(repo), "add", "issues/index.jsonl"],
                   check=True, capture_output=True, text=True, env=env)
    subprocess.run(["git", "-C", str(repo), "-c", "commit.gpgsign=false",
                    "commit", "-m", "x"],
                   check=True, capture_output=True, text=True, env=env)


def _row(iid, **extra):
    row = {"id": iid, "slug": f"i{iid}", "title": f"I{iid}", "kind": "task",
           "status": "backlog", "priority": "medium"}
    row.update(extra)
    return json.dumps(row, ensure_ascii=False)


@unittest.skipUnless(shutil.which("git"), "git not available")
class TestGitLayer(unittest.TestCase):
    def setUp(self):
        self.b = load_backfill()

    def test_resolve_index_missing_file(self):
        with TemporaryDirectory() as tmp:
            with self.assertRaises(SystemExit):
                self.b.resolve_index(tmp)

    def test_resolve_index_not_a_repo(self):
        with TemporaryDirectory() as tmp:
            (Path(tmp) / "index.jsonl").write_text("")
            with self.assertRaises(SystemExit):
                self.b.resolve_index(tmp)

    def test_recover_times_over_history(self):
        with TemporaryDirectory() as tmp:
            _git(tmp, "init", "-q")
            _commit_index(tmp, _row(1, created="2026-06-01") + "\n",
                          "2026-06-01T09:00:00+00:00")
            _commit_index(tmp, _row(1, created="2026-06-01", started="2026-06-02",
                                    status="ongoing") + "\n",
                          "2026-06-02T12:00:00-03:00")
            _commit_index(tmp, _row(1, created="2026-06-01", started="2026-06-02",
                                    closed="2026-06-03", status="done") + "\n",
                          "2026-06-03T10:00:00+00:00")
            root, index_rel, _ = self.b.resolve_index(str(Path(tmp) / "issues"))
            rec = self.b.recover_times(root, index_rel)
            self.assertEqual(rec[(1, "created")], "2026-06-01T09:00:00+00:00")
            self.assertEqual(rec[(1, "started")], "2026-06-02T12:00:00-03:00")
            self.assertEqual(rec[(1, "closed")], "2026-06-03T10:00:00+00:00")
```

- [ ] **Step 2: Run to verify they fail**

Run: `python3 -m unittest tests.test_backfill.TestGitLayer -v`
Expected: FAIL — `AttributeError: module 'backfill_timestamps' has no attribute 'resolve_index'`.

- [ ] **Step 3: Implement the git layer**

In `scripts/backfill_timestamps.py`, add after `reduce_transitions`:

```python
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
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `python3 -m unittest tests.test_backfill.TestGitLayer -v`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add scripts/backfill_timestamps.py tests/test_backfill.py
git commit -m "backfill: git layer to recover field times from history"
```

---

### Task 5: `backfill()` + `main()` + end-to-end test

**Files:**
- Modify: `scripts/backfill_timestamps.py`
- Test: `tests/test_backfill.py`

- [ ] **Step 1: Write the failing tests**

Append to `tests/test_backfill.py` (reuses `_git`, `_commit_index`, `_row` from Task 4):

```python
@unittest.skipUnless(shutil.which("git"), "git not available")
class TestMainEndToEnd(unittest.TestCase):
    def setUp(self):
        self.b = load_backfill()

    def _build_repo(self, tmp):
        """Issue 1: created, started, closed, reopened, reclosed. Working tree =
        final state with all three fields day-only."""
        _git(tmp, "init", "-q")
        _commit_index(tmp, _row(1, created="2026-06-01") + "\n",
                      "2026-06-01T09:00:00+00:00")
        _commit_index(tmp, _row(1, created="2026-06-01", started="2026-06-02",
                                status="ongoing") + "\n",
                      "2026-06-02T12:00:00-03:00")
        _commit_index(tmp, _row(1, created="2026-06-01", started="2026-06-02",
                                closed="2026-06-03", status="done") + "\n",
                      "2026-06-03T10:00:00+00:00")
        _commit_index(tmp, _row(1, created="2026-06-01", started="2026-06-02",
                                status="ongoing") + "\n",   # reopened
                      "2026-06-04T10:00:00+00:00")
        _commit_index(tmp, _row(1, created="2026-06-01", started="2026-06-02",
                                closed="2026-06-05", status="done") + "\n",   # reclosed
                      "2026-06-05T22:30:00-03:00")
        return str(Path(tmp) / "issues")

    def test_main_backfills_to_expected_utc_and_is_idempotent(self):
        with TemporaryDirectory() as tmp:
            tracker = self._build_repo(tmp)
            index = Path(tracker) / "index.jsonl"

            rc = self.b.main([tracker])
            self.assertEqual(rc, 0)
            row = json.loads(index.read_text().splitlines()[0])
            self.assertEqual(row["created"], "2026-06-01T09:00:00Z")
            self.assertEqual(row["started"], "2026-06-02T15:00:00Z")
            self.assertEqual(row["closed"], "2026-06-06T01:30:00Z")  # 22:30-03:00 -> next day UTC

            # second run changes nothing (idempotent)
            before = index.read_text()
            self.b.main([tracker])
            self.assertEqual(index.read_text(), before)

    def test_dry_run_writes_nothing(self):
        with TemporaryDirectory() as tmp:
            tracker = self._build_repo(tmp)
            index = Path(tracker) / "index.jsonl"
            before = index.read_text()
            self.b.main([tracker, "--dry-run"])
            self.assertEqual(index.read_text(), before)  # still day-only, unchanged

    def test_main_runs_as_subprocess(self):
        with TemporaryDirectory() as tmp:
            tracker = self._build_repo(tmp)
            from tests.helpers import SCRIPT_PATH
            r = subprocess.run([sys.executable, str(SCRIPT_PATH), tracker],
                               capture_output=True, text=True)
            self.assertEqual(r.returncode, 0, r.stderr)
            row = json.loads((Path(tracker) / "index.jsonl").read_text().splitlines()[0])
            self.assertEqual(row["created"], "2026-06-01T09:00:00Z")
```

- [ ] **Step 2: Run to verify they fail**

Run: `python3 -m unittest tests.test_backfill.TestMainEndToEnd -v`
Expected: FAIL — `AttributeError: module 'backfill_timestamps' has no attribute 'main'`.

- [ ] **Step 3: Implement `backfill` and `main`**

In `scripts/backfill_timestamps.py`, add after `recover_times`:

```python
def backfill(index_path, recovered, dry_run):
    """Rewrite the working-tree index, applying recovered timestamps to day-only
    date fields. Returns (changes, warnings). Writes only when not dry_run and
    something actually changed."""
    lines = Path(index_path).read_text().splitlines()
    new_lines, changes, warnings = rewrite_lines(lines, recovered)
    if not dry_run and changes:
        Path(index_path).write_text("\n".join(new_lines) + "\n")
    return changes, warnings


def main(argv=None):
    parser = argparse.ArgumentParser(
        description="Backfill day-only trck dates with UTC timestamps recovered "
                    "from git history.")
    parser.add_argument("tracker_dir", nargs="?", default="issues",
                        help="tracker dir containing index.jsonl (default: issues)")
    parser.add_argument("--dry-run", action="store_true",
                        help="print the planned changes but do not write")
    args = parser.parse_args(argv)

    root, index_rel, index_path = resolve_index(args.tracker_dir)
    recovered = recover_times(root, index_rel)
    try:
        changes, warnings = backfill(index_path, recovered, args.dry_run)
    except json.JSONDecodeError as e:
        sys.exit(f"error: malformed JSON in {index_path}: {e}")

    for iid, field, old, new in changes:
        print(f"#{iid:03d} {field}: {old} -> {new}")
    for iid, field, old in warnings:
        print(f"WARNING: #{iid:03d} {field} day-only but no history found ({old})")
    note = "dry-run, no changes written; " if args.dry_run else ""
    print(f"{note}{len(changes)} field(s) updated, {len(warnings)} warning(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `python3 -m unittest tests.test_backfill.TestMainEndToEnd -v`
Expected: PASS (3 tests). The closed value `2026-06-06T01:30:00Z` confirms both the reopen→reclose "last wins" rule and the day-boundary UTC conversion.

- [ ] **Step 5: Commit**

```bash
git add scripts/backfill_timestamps.py tests/test_backfill.py
git commit -m "backfill: in-place write, CLI, and report (main)"
```

---

### Task 6: Full-suite regression + real-repo dry-run smoke

**Files:** none (verification only)

- [ ] **Step 1: Run the entire suite**

Run: `python3 -m unittest discover -s tests`
Expected: PASS — all modules green, including the new `tests.test_backfill`.

- [ ] **Step 2: Dry-run against this repo's own tracker (read-only)**

Run: `python3 scripts/backfill_timestamps.py issues --dry-run`
Expected: a report of the day-only fields it WOULD convert (this repo's existing issues are day-only), ending with `dry-run, no changes written; N field(s) updated, M warning(s)`. Confirm with `git status --porcelain issues/index.jsonl` that the file is unchanged (no output).

- [ ] **Step 3: Confirm the script is stdlib-only and engine-free**

Run: `grep -nE "^(import|from) " scripts/backfill_timestamps.py`
Expected: only `argparse`, `json`, `re`, `subprocess`, `sys`, `datetime`, `pathlib` — no third-party imports and no import of `trck`.

- [ ] **Step 4: Commit (only if Step 1-3 required a fix)**

If a fix was needed, commit it; otherwise skip.

```bash
git add -A
git commit -m "backfill: fix surfaced by full-suite/smoke verification"
```

---

## Self-Review Notes

- **Spec coverage:** stdlib-only standalone script, no engine import (Task 1, verified Task 6 Step 3) ✓; invocation `python3 scripts/backfill_timestamps.py [TRACKER_DIR] [--dry-run]`, default `issues` (Task 5) ✓; recover from author date of the last commit setting the field (Task 3 reducer + Task 4 `recover_times`) ✓; rewrite only day-only values, idempotent (Task 2 guard + Task 5 idempotency test) ✓; byte-identical unchanged lines (Task 2 test) ✓; `--dry-run` writes nothing (Task 5 test) ✓; clean errors for no-git / not-a-repo / missing-index / malformed-JSON (Task 4 `resolve_index`/`_git`, Task 5 `main` JSON guard) ✓; unresolved day-only fields warned not invented (Task 2 warning test) ✓; unit + git-backed integration incl. reopen→reclose and idempotency (Tasks 1-5) ✓.
- **Placeholder scan:** none — every code/test step contains complete code.
- **Name consistency across tasks:** `to_utc`, `is_day_only`, `rewrite_lines`, `reduce_transitions`, `_git`, `resolve_index`, `list_history`, `read_index_at`, `recover_times`, `backfill`, `main`, `DATE_FIELDS`, `DAY_ONLY_RE` — used identically wherever they appear; test helpers `_git`/`_commit_index`/`_row` are defined in Task 4 and reused in Task 5.
- **Test isolation:** every test class loads a fresh module via `load_backfill()` in `setUp`; git-backed classes are `@unittest.skipUnless(shutil.which("git"), ...)`; all commits set explicit author/committer identity + date env so they don't depend on global git config.
