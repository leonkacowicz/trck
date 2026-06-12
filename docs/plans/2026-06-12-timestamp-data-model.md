# Timestamp Data Model Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Convert the trck engine to stamp `created`/`started`/`closed` as full UTC ISO timestamps while still reading legacy day-only values, so a release's own timestamp can later serve as a `--since` filter.

**Architecture:** Replace the day-only `today()` helper with a second-precision `Z`-suffixed `now_utc()`; point the three stamp sites at it; add a `date_slice()` helper so human-facing renders still show a bare `YYYY-MM-DD`; rely on zero-padded-ISO string ordering so comparison/sort paths need no parsing and legacy values keep working.

**Tech Stack:** Python 3 standard library only (`datetime`, `unittest`). Single-file engine `./trck`; tests under `tests/` run via `python3 -m unittest`.

**Spec:** `docs/specs/2026-06-12-timestamp-data-model-design.md`

---

## File Structure

- **`trck`** (the engine) — all production changes:
  - `trck:20` import line: `from datetime import date` → `from datetime import datetime, timezone`.
  - `trck:38-39` `today()` → `now_utc()` (new timestamp producer).
  - New `date_slice()` helper near `now_utc()` (sole human-display formatter).
  - `trck:1237`, `trck:1240`, `trck:1274` stamp sites call `now_utc()`.
  - `trck:1101-1102` SUMMARY closed annotation uses `date_slice()`.
  - `trck:1416-1421` `cmd_show` human branch slices the three date fields.
- **`tests/test_basics.py`** — update the `today()` format test to `now_utc()`.
- **`tests/test_lifecycle.py`** — update the three day-only stamp assertions to expect timestamps (via a stubbed `now_utc`).
- **`tests/test_timestamps.py`** (new) — format, read-both/back-compat, mixed-form sort, and display tests.

Note: `today()` has exactly one definition and three callers (`trck:1237/1240/1274`) — confirmed by `grep -n "today()" trck`. No other module references it except `tests/test_basics.py`.

---

### Task 1: `now_utc()` helper replaces `today()`

**Files:**
- Modify: `trck:20` (import), `trck:38-39` (`today` → `now_utc`)
- Test: `tests/test_basics.py:22`

- [ ] **Step 1: Update the failing test**

In `tests/test_basics.py`, replace the existing `today()` format test (line ~22) so it targets the new name and the timestamp shape:

```python
    def test_now_utc_is_zsuffixed_second_precision(self):
        self.assertRegex(self.t.now_utc(), r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$")
```

(If the surrounding test method had a different name like `test_today_*`, rename it to the above. There must be no remaining reference to `self.t.today` in the file — grep to confirm in Step 2.)

- [ ] **Step 2: Run it to verify it fails**

Run: `python3 -m unittest tests.test_basics -v`
Expected: FAIL — `AttributeError: module 'trck_engine' has no attribute 'now_utc'`.
Also run `grep -n "today" tests/test_basics.py` and confirm no `self.t.today` reference remains.

- [ ] **Step 3: Implement `now_utc()` and swap the import**

In `trck`, change the import at line 20:

```python
from datetime import datetime, timezone
```

Replace the `today()` definition (lines 38-39) with:

```python
def now_utc() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `python3 -m unittest tests.test_basics -v`
Expected: PASS for `test_now_utc_is_zsuffixed_second_precision`.

Note: the full suite will still have failures in `tests.test_lifecycle` (it calls `today()` indirectly through the unchanged stamp sites, which now reference a missing `today`). That is fixed in Task 2. Do not "fix" it here.

- [ ] **Step 5: Commit**

```bash
git add trck tests/test_basics.py
git commit -m "engine: now_utc() UTC-timestamp helper replaces today()"
```

---

### Task 2: Stamp `created`/`started`/`closed` as timestamps

**Files:**
- Modify: `trck:1237`, `trck:1240`, `trck:1274` (`today()` → `now_utc()`)
- Test: `tests/test_lifecycle.py`

- [ ] **Step 1: Update the failing tests**

In `tests/test_lifecycle.py`, the three assertions currently compare against `date.today().isoformat()` (lines ~32, 43, 53). Make stamping deterministic by stubbing `now_utc` and asserting equality. Add a stub helper to the test class and rewrite the three assertions.

Add this stub at the top of the relevant tests (or in `setUp`), then use it before the mutation that stamps:

```python
    def stub_now(self, value="2026-06-12T10:00:00Z"):
        self.t.now_utc = lambda: value
        return value
```

Rewrite the three affected tests' assertions:

```python
    def test_new_lands_in_initial_status(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            ts = self.stub_now()
            self.new(d)
            r = self.rows(d)[0]
            self.assertEqual(r.status, "backlog")
            self.assertEqual(r.created, ts)
            self.assertIsNone(r.started)
            self.assertTrue((d / "backlog" / "001-first.md").exists())

    def test_mv_stamps_started_on_leaving_initial(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            ts = self.stub_now()
            self.new(d)
            self.t.cmd_mv(ns(dir=str(d), id=1, status="ongoing", resolution=None))
            r = self.rows(d)[0]
            self.assertEqual(r.status, "ongoing")
            self.assertEqual(r.started, ts)
            self.assertIsNone(r.closed)
            self.assertTrue((d / "ongoing" / "001-first.md").exists())

    def test_mv_to_terminal_stamps_closed_and_resolution(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            ts = self.stub_now()
            self.new(d)
            self.t.cmd_mv(ns(dir=str(d), id=1, status="done", resolution="wontfix"))
            r = self.rows(d)[0]
            self.assertEqual(r.closed, ts)
            self.assertEqual(r.resolution, "wontfix")
```

`self.t.now_utc = ...` reassigns the module global the stamp sites look up at call time, so the stub takes effect. The `from datetime import date` import in this file is now unused — remove that import line. (Run `grep -n "date" tests/test_lifecycle.py` after editing to confirm nothing else needs it.)

- [ ] **Step 2: Run the tests to verify they fail**

Run: `python3 -m unittest tests.test_lifecycle -v`
Expected: FAIL — the stamp sites still call `today()` (now undefined), raising `NameError: name 'today' is not defined` when creating/moving.

- [ ] **Step 3: Point the stamp sites at `now_utc()`**

In `trck`, change all three call sites from `today()` to `now_utc()`:

- Line ~1237: `row.started = now_utc()`
- Line ~1240: `row.closed = now_utc()`
- Line ~1274: `... spec=args.spec, created=now_utc(),`

After editing, `grep -n "today()" trck` must return nothing.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `python3 -m unittest tests.test_lifecycle -v`
Expected: PASS (all lifecycle tests).

- [ ] **Step 5: Commit**

```bash
git add trck tests/test_lifecycle.py
git commit -m "engine: stamp created/started/closed as UTC timestamps"
```

---

### Task 3: `date_slice()` for human-facing renders

**Files:**
- Modify: `trck` (new `date_slice()` near `now_utc()`); SUMMARY render `trck:1101-1102`; `cmd_show` human branch `trck:1416-1421`
- Test: `tests/test_timestamps.py` (new file)

- [ ] **Step 1: Write the failing tests**

Create `tests/test_timestamps.py`:

```python
import json
import unittest
from io import StringIO
from contextlib import redirect_stdout
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestTimestampDisplay(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def setup_dir(self, tmp):
        return make_tracker(tmp, {})

    def new(self, d, title="First"):
        self.t.cmd_new(ns(dir=str(d), title=title, priority="high", kind=None,
                          parent=None, depends=None, spec=None, slug=None))

    def rows(self, d):
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return self.t.load_index(ctx)

    def test_date_slice_trims_timestamp_to_date(self):
        self.assertEqual(self.t.date_slice("2026-06-12T10:00:00Z"), "2026-06-12")

    def test_date_slice_passes_through_legacy_date(self):
        self.assertEqual(self.t.date_slice("2026-06-05"), "2026-06-05")

    def test_date_slice_handles_none(self):
        self.assertEqual(self.t.date_slice(None), "")

    def test_summary_shows_bare_date_for_closed(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            self.t.now_utc = lambda: "2026-06-12T10:00:00Z"
            self.new(d)
            self.t.cmd_mv(ns(dir=str(d), id=1, status="done", resolution=None))
            text = (d / "SUMMARY.md").read_text()
            self.assertIn("(closed 2026-06-12)", text)
            self.assertNotIn("2026-06-12T10:00:00Z", text)

    def test_show_human_view_slices_dates_but_json_keeps_full(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            self.t.now_utc = lambda: "2026-06-12T10:00:00Z"
            self.new(d)
            # human view: bare date
            buf = StringIO()
            with redirect_stdout(buf):
                self.t.cmd_show(ns(dir=str(d), id=1, json=False))
            human = buf.getvalue()
            self.assertRegex(human, r"created\s+2026-06-12\b")
            self.assertNotIn("2026-06-12T10:00:00Z", human)
            # json view: full timestamp
            buf = StringIO()
            with redirect_stdout(buf):
                self.t.cmd_show(ns(dir=str(d), id=1, json=True))
            payload = json.loads(buf.getvalue())
            self.assertEqual(payload["created"], "2026-06-12T10:00:00Z")


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `python3 -m unittest tests.test_timestamps -v`
Expected: FAIL — `AttributeError: module 'trck_engine' has no attribute 'date_slice'` (and the show/summary assertions fail because the full timestamp is still rendered).

- [ ] **Step 3: Add `date_slice()` and apply it to both render sites**

In `trck`, add directly below `now_utc()`:

```python
def date_slice(ts: str | None) -> str:
    return ts[:10] if ts else ""
```

In the SUMMARY render (lines ~1101-1102), change:

```python
            if is_terminal(ctx.cfg, status) and r.closed:
                extra += f" (closed {date_slice(r.closed)})"
```

In `cmd_show`'s human branch (the `else:` loop around lines ~1416-1421), slice the three date fields when printing the human view. Replace the loop body so date fields render their slice:

```python
        w = max(len(k) for k in keys)
        for k in keys:
            v = full.get(k)
            if v is None or v == []:
                continue  # skip empty fields in the human view
            if k in ("created", "started", "closed"):
                v = date_slice(v)
            print(f"{paint(f'{k:>{w}}', 'dim')}  {v}")
```

The `--json` branch (line ~1413) is untouched, so it still emits the full timestamp.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `python3 -m unittest tests.test_timestamps -v`
Expected: PASS (all five tests).

- [ ] **Step 5: Commit**

```bash
git add trck tests/test_timestamps.py
git commit -m "engine: render created/started/closed as a bare date in human views"
```

---

### Task 4: Read-both back-compat and mixed-form sort

**Files:**
- Test only: `tests/test_timestamps.py` (extend)
- No production change expected — this task proves legacy values still work and locks the behavior in.

- [ ] **Step 1: Write the failing/locking tests**

Append a second test class to `tests/test_timestamps.py`:

```python
class TestTimestampBackCompat(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def setup_dir(self, tmp):
        return make_tracker(tmp, {})

    def write_index(self, d, lines):
        (d / "index.jsonl").write_text("".join(json.dumps(x) + "\n" for x in lines))

    def ctx(self, d):
        return self.t.Ctx(d, self.t.load_config(d))

    def test_legacy_date_only_loads_check_clean_and_normalize_preserves(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            (d / "backlog").mkdir()
            # create the issue body file the index points at
            (d / "backlog" / "001-legacy.md").write_text("# Legacy\n")
            self.write_index(d, [{
                "id": 1, "slug": "legacy", "title": "Legacy", "kind": "task",
                "status": "backlog", "priority": "medium",
                "created": "2026-06-05",  # legacy day-only value
            }])
            ctx = self.ctx(d)
            rows = self.t.load_index(ctx)          # loads without error
            self.assertEqual(rows[0].created, "2026-06-05")
            errors, _ = self.t.validate(ctx, rows)  # check passes
            self.assertEqual(errors, [])
            # normalize must NOT expand the date-only value
            self.t.cmd_normalize(ns(dir=str(d)))
            reloaded = self.t.load_index(self.ctx(d))
            self.assertEqual(reloaded[0].created, "2026-06-05")

    def test_mixed_form_sort_orders_date_before_same_day_timestamp(self):
        with TemporaryDirectory() as tmp:
            d = self.setup_dir(tmp)
            (d / "backlog").mkdir()
            (d / "backlog" / "001-older.md").write_text("# Older\n")
            (d / "backlog" / "002-newer.md").write_text("# Newer\n")
            self.write_index(d, [
                {"id": 1, "slug": "older", "title": "Older", "kind": "task",
                 "status": "backlog", "priority": "medium", "created": "2026-06-05"},
                {"id": 2, "slug": "newer", "title": "Newer", "kind": "task",
                 "status": "backlog", "priority": "medium",
                 "created": "2026-06-05T09:00:00Z"},
            ])
            rows = self.t.load_index(self.ctx(d))
            ordered = sorted(rows, key=lambda r: (r.created or "", r.id))
            self.assertEqual([r.id for r in ordered], [1, 2])
```

- [ ] **Step 2: Run the tests to verify behavior**

Run: `python3 -m unittest tests.test_timestamps -v`
Expected: PASS. If `cmd_normalize`'s Namespace needs more attributes than `dir`, the run will raise `AttributeError`; add the missing attribute to the `ns(...)` call (check the `normalize` subparser around `trck:2041+` for its argument set) and re-run. No engine change should be required for these to pass — if one fails because `normalize` *rewrites* the date, that is a real regression to investigate, not a test to weaken.

- [ ] **Step 3: Commit**

```bash
git add tests/test_timestamps.py
git commit -m "test: lock legacy date-only read/normalize and mixed-form sort"
```

---

### Task 5: Full-suite regression check

**Files:** none (verification only)

- [ ] **Step 1: Run the entire suite**

Run: `python3 -m unittest discover -s tests -v`
Expected: PASS — all modules green. Pay attention to `test_summary`, `test_presentation`, `test_read`, and `test_metadata`, which exercise `show`/SUMMARY rendering and could surface a missed full-timestamp leak.

- [ ] **Step 2: Self-host check**

Run: `./trck check`
Expected: clean exit (the real `issues/` tracker still validates; its existing day-only values are read fine).

- [ ] **Step 3: Confirm no stragglers**

Run: `grep -n "today()" trck tests/*.py`
Expected: no output. `today` is fully retired.

- [ ] **Step 4: Commit (only if any fixup was needed)**

If steps 1-3 required a fix, commit it:

```bash
git add -A
git commit -m "engine: fix timestamp-render straggler surfaced by full suite"
```

If nothing needed fixing, skip this commit.

---

## Self-Review Notes

- **Spec coverage:** `now_utc()` format (Task 1) ✓; stamp sites (Task 2) ✓; read-both + normalize-preserves + check-clean (Task 4) ✓; mixed-form sort (Task 4) ✓; date-slice display in show + SUMMARY, with `--json` keeping the full value (Task 3) ✓; full-suite green (Task 5) ✓.
- **No format validation added** — consistent with the spec's "stay lenient" decision; Task 4 proves legacy values are accepted, no task adds a rejecting check.
- **Names are consistent across tasks:** `now_utc()`, `date_slice()` used identically everywhere they appear.
- **Out of scope (later sub-projects), not in this plan:** git-history backfill script; the `--since` release-notes verb; any `--json` epic work beyond the already-existing `show --json`.
