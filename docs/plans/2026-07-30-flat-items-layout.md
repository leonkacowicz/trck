# Flat `items/` Layout Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Stop encoding issue status in the filesystem — move every issue body into a single flat `items/` directory so `index.jsonl` is the sole source of truth for status.

**Architecture:** The path becomes `<tracker>/items/<id>-<slug>.md` instead of `<tracker>/<status>/<id>-<slug>.md`. Nearly all of the change funnels through three functions in `src/trck/index.py` (`filename`, `issue_path`, `rel_link`) plus `scan_files` in `src/trck/scan.py`. Status changes become pure index edits: `move_issue` loses its `shutil.move`, and `validate` loses the index-status-vs-folder drift check that only existed because of the duplication. Because the path is still id+slug derived, `trck set --slug` still renames the file — the deliberate, rare case — while `start`/`review`/`done` stop touching the working tree entirely. Existing trackers are detected on every command and refused until a one-shot `trck repo migrate-layout` verb relocates their files.

**Tech Stack:** Python 3, standard library only. Source of truth is `src/trck/*.py`; `./trck` is a generated amalgamation produced by `python3 build.py`.

## Global Constraints

- **Standard library only.** No third-party imports, ever.
- **Never hand-edit `./trck`.** It is generated. Edit `src/trck/*.py` and run `python3 build.py`. The pre-commit hook runs `build.py --check` and rejects drift.
- **`tests/__init__.py` rebuilds `./trck` from `src/` before the suite runs**, so tests always reflect source edits. Commit the regenerated `./trck` alongside every source change.
- **Module order matters** (`build.py::MANIFEST`): `constants → config → index → graph → scan → render → summary → finalize → net → templates → cmd_mutate → cmd_query → cmd_maint → cmd_selfmgmt → cli`. A module-level constant must be defined before a later module's *top-level* code uses it. Function bodies resolve at call time, so cross-module calls in either direction are fine.
- **Keep editor imports in sync.** `src/trck/` is build input, not a runnable package. When you reference a new sibling symbol, add the matching `from .mod import name` — the build strips it, but pyright/Pylance need it.
- **The vocabulary is data-driven.** Never hard-code status, priority, or kind names in the engine. Read them from config via `status_names`/`check_*` helpers.
- **The body directory name is `items`.** Exposed as `ITEMS_DIR` in `src/trck/constants.py`. Never write the bare string `"items"` anywhere else in the engine. It is *not* a reserved status name — statuses no longer name directories, so a status called `items` is harmless and must stay legal.
- **Run the full suite** with `python3 -m unittest discover -s tests -v`; a single module with `python3 -m unittest tests.test_paths -v`.
- **Target version: `0.23.0`** (breaking on-disk format change), bumped in `src/trck/constants.py`.

---

## File Structure

**Modified — engine:**

| File | Responsibility after this change |
|---|---|
| `src/trck/constants.py` | Adds `ITEMS_DIR`; bumps `__version__` to `0.23.0`. |
| `src/trck/config.py` | Adds `detect_legacy_layout(cfg, tracker_dir)`. |
| `src/trck/index.py` | `issue_path`/`rel_link` point into `items/`; `build_ctx_or_die` grows a `guard_layout` parameter that refuses a legacy-layout tracker. |
| `src/trck/graph.py` | `_existing_ids` globs `items/` instead of every status folder; drops the now-unused `status_names` import. |
| `src/trck/scan.py` | `scan_files` walks one directory and returns `id -> (slug, filename)`; `validate` drops the status-vs-folder check. |
| `src/trck/templates.py` | `move_issue` becomes status assignment + date stamping only (no `shutil.move`); `CLAUDE_MD_TEMPLATE` stops describing status as a folder. |
| `src/trck/cmd_maint.py` | Adds `cmd_migrate_layout`. |
| `src/trck/cli.py` | Registers `migrate-layout` under the `trck repo` group (see tracker issue #qs4zwzr, a prerequisite). |

**Modified — tests:** `tests/test_paths.py` (one literal path), `tests/test_validate.py` (drop the folder-mismatch test).

**Created — tests:** `tests/test_layout.py` — the flat-layout contract, the legacy-layout guard, and `migrate-layout` end to end.

**Modified — docs & data:** `README.md`, `CLAUDE.md`, `issues/CLAUDE.md`, this repo's own `issues/` tree, `examples/action-game/`.

---

### Task 1: Flip the layout — paths, scan, and validation

The core change. After this task new issues land in `items/`, `scan_files` reads them from there, and the status-vs-folder drift check is gone. `move_issue`'s `shutil.move` becomes dead code (old and new paths are now equal, so the `if` never fires) — Task 2 removes it.

**Files:**
- Modify: `src/trck/constants.py:13` (add `ITEMS_DIR` after `FILENAME_RE`)
- Modify: `src/trck/index.py:1-7` (import), `src/trck/index.py:225-230` (`rel_link`, `issue_path`)
- Modify: `src/trck/scan.py:1-5` (imports), `src/trck/scan.py:10-25` (`scan_files`), `src/trck/scan.py:44-54` (`validate` unpacking)
- Modify: `src/trck/graph.py:3` (import), `src/trck/graph.py:541-552` (`_existing_ids`)
- Create: `tests/test_layout.py`
- Modify: `tests/test_paths.py:122`
- Modify: `tests/test_validate.py:36-44` (delete `test_status_folder_mismatch_is_error`)

**Interfaces:**
- Produces: `ITEMS_DIR: str` in `constants` — the single directory name, `"items"`. Every later task imports it rather than writing the literal.
- Produces: `scan_files(ctx) -> dict[str, tuple[str, str]]` mapping id to `(slug, filename)`. The old 3-tuple with a leading folder name is gone; `validate` is its only caller.
- Produces: `issue_path(ctx, row) -> Path` == `ctx.dir / ITEMS_DIR / filename(row)` and `rel_link(row) -> str` == `f"{ITEMS_DIR}/{filename(row)}"`.

---

- [ ] **Step 1: Write the failing tests**

Create `tests/test_layout.py`:

```python
"""The flat items/ layout: status lives only in index.jsonl, never in the path."""
import io
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestFlatLayout(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title="Item", **over):
        a = dict(dir=str(d), title=title, priority="high", kind=None, parent=None,
                 points=None, depends=None, spec=None, slug=None, pr=None)
        a.update(over)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(ns(**a))
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def test_new_writes_into_items_dir(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha")
            files = sorted(p.name for p in (d / "items").glob("*.md"))
            self.assertEqual(len(files), 1)
            self.assertTrue(files[0].endswith("-alpha.md"))
            self.assertFalse((d / "backlog").exists())

    def test_status_change_does_not_move_the_file(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            before = sorted(p.name for p in (d / "items").glob("*.md"))
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_mv(ns(dir=str(d), id=iid, status="done",
                                 resolution=None, pr=None))
            after = sorted(p.name for p in (d / "items").glob("*.md"))
            self.assertEqual(before, after)
            self.assertFalse((d / "done").exists())
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            row = self.t.get_row(self.t.load_index(ctx), iid)
            self.assertEqual(row.status, "done")

    def test_rel_link_points_into_items_dir(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            row = self.t.get_row(self.t.load_index(ctx), iid)
            self.assertEqual(self.t.rel_link(row), f"items/{self.t.filename(row)}")
            self.assertIn(f"(items/{self.t.filename(row)})",
                          (d / "SUMMARY.md").read_text())

    def test_slug_change_still_renames_within_items(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_set(ns(dir=str(d), id=iid, slug="renamed", title=None,
                                  priority=None, points=None, parent=None, spec=None,
                                  pr=None, kind=None, field=None, unset=None, auto=False))
            files = sorted(p.name for p in (d / "items").glob("*.md"))
            self.assertEqual(files, [f"{iid}-renamed.md"])

    def test_scan_files_maps_id_to_slug_and_filename(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            found = self.t.scan_files(ctx)
            self.assertEqual(found[iid], ("alpha", f"{iid}-alpha.md"))
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `python3 -m unittest tests.test_layout -v`
Expected: FAIL — `test_new_writes_into_items_dir` errors because `d / "items"` does not exist (files land in `backlog/`), and `test_scan_files_maps_id_to_slug_and_filename` fails on a 3-tuple.

- [ ] **Step 3: Add the `ITEMS_DIR` constant**

In `src/trck/constants.py`, immediately after the `FILENAME_RE` line (line 13), add:

```python
ITEMS_DIR = "items"  # the one directory holding every issue body; status lives in index.jsonl
```

- [ ] **Step 4: Point the path helpers at `items/`**

In `src/trck/index.py`, extend the constants import on line 7 to:

```python
from .constants import FIELD_KEY_RE, ITEMS_DIR, die
```

Then replace `rel_link` and `issue_path` (lines 225-230) with:

```python
def rel_link(row: Issue) -> str:
    return f"{ITEMS_DIR}/{filename(row)}"


def issue_path(ctx: Ctx, row: Issue) -> Path:
    return ctx.dir / ITEMS_DIR / filename(row)
```

- [ ] **Step 5: Make `scan_files` walk one directory**

In `src/trck/scan.py`, replace the constants import on line 3 with:

```python
from .constants import FIELD_KEY_RE, FILENAME_RE, ITEMS_DIR, SLUG_RE, die
```

Replace `scan_files` (lines 10-25) with:

```python
def scan_files(ctx: Ctx) -> dict:
    """Map id -> (slug, filename) for every issue markdown in the items dir. Status
    is not encoded in the path, so the folder component the old layout returned is
    gone; two files can still claim one id via different slugs, which is fatal."""
    found = {}
    d = ctx.dir / ITEMS_DIR
    if not d.is_dir():
        return found
    for p in sorted(d.glob("*.md")):
        m = FILENAME_RE.match(p.name)
        if not m:
            continue
        iid = file_id(m)
        if iid in found:
            die(f"duplicate issue id {iid} on disk: {found[iid][1]} and {p.name}")
        found[iid] = (m.group(2), p.name)
    return found
```

- [ ] **Step 6: Drop the status-vs-folder drift check from `validate`**

In `src/trck/scan.py`, replace lines 48-50 — the block reading:

```python
        folder, slug, fname = files[iid]
        if r.status != folder:
            errors.append(f"#{iid} index status '{r.status}' != folder '{folder}'")
```

with:

```python
        slug, fname = files[iid]
```

Leave the slug and filename checks that follow untouched — they still guard the one piece of metadata the path does encode.

- [ ] **Step 7: Point `_existing_ids` at `items/`**

In `src/trck/graph.py`, replace the config import on line 3 with (dropping `status_names`, now unused in this module):

```python
from .config import is_actionable, is_terminal, reconcile
```

Add `ITEMS_DIR` to the constants import in the same header block, then replace `_existing_ids` (lines 541-552) with:

```python
def _existing_ids(ctx: Ctx) -> set[str]:
    """Every id currently visible: index rows ∪ on-disk filenames."""
    ids = {r.id for r in load_index(ctx)}
    d = ctx.dir / ITEMS_DIR
    if d.is_dir():
        for p in d.glob("*.md"):
            m = FILENAME_RE.match(p.name)
            if m:
                ids.add(file_id(m))
    return ids
```

- [ ] **Step 8: Fix the two layout-coupled existing tests**

In `tests/test_paths.py:122`, change:

```python
            rel = f"issues/backlog/{fname}"
```

to:

```python
            rel = f"issues/items/{fname}"
```

In `tests/test_validate.py`, delete `test_status_folder_mismatch_is_error` entirely (lines 36-44) — the drift it asserted is now unrepresentable.

- [ ] **Step 9: Rebuild and run the full suite**

Run: `python3 build.py && python3 -m unittest discover -s tests -v`
Expected: PASS, all tests green.

- [ ] **Step 10: Commit**

```bash
git add src/trck/constants.py src/trck/index.py src/trck/scan.py src/trck/graph.py \
        tests/test_layout.py tests/test_paths.py tests/test_validate.py trck
git commit -m "layout: put every issue body in a flat items/ dir

Status stops being encoded in the filesystem: issue_path and rel_link
resolve to <tracker>/items/<id>-<slug>.md, scan_files walks that one
directory, and validate loses the index-status-vs-folder check that only
existed to catch the duplication drifting.

The path still carries id and slug, so a deliberate 'set --slug' still
renames the file; start/review/done no longer touch the working tree."
```

---

### Task 2: Strip the dead file move out of `move_issue`

With the path no longer status-derived, `move_issue`'s `old.resolve() != new.resolve()` is always false and the `shutil.move` is unreachable. Remove it, keep the loud missing-file guard.

**Files:**
- Modify: `src/trck/templates.py:1-6` (imports), `src/trck/templates.py:164-188` (`move_issue`)
- Modify: `tests/test_layout.py` (add the guard test)

**Interfaces:**
- Consumes: `ITEMS_DIR`, `issue_path` from Task 1.
- Produces: `move_issue(ctx, row, new_status) -> None` — same signature, now pure status assignment plus date stamping. Callers (`cmd_mv`, `normalize_statuses`) are unchanged.

---

- [ ] **Step 1: Write the failing test**

Append to `TestFlatLayout` in `tests/test_layout.py`:

```python
    def test_move_issue_dies_when_the_body_file_is_missing(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            rows = self.t.load_index(ctx)
            row = self.t.get_row(rows, iid)
            self.t.issue_path(ctx, row).unlink()
            with self.assertRaises(SystemExit):
                self.t.move_issue(ctx, row, "done")

    def test_move_issue_stamps_dates_without_touching_the_file(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            iid = self.seed(d, "Alpha")
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            row = self.t.get_row(self.t.load_index(ctx), iid)
            path = self.t.issue_path(ctx, row)
            mtime = path.stat().st_mtime_ns
            self.t.move_issue(ctx, row, "done")
            self.assertEqual(row.status, "done")
            self.assertIsNotNone(row.started)
            self.assertIsNotNone(row.closed)
            self.assertEqual(path.stat().st_mtime_ns, mtime)  # body untouched
```

- [ ] **Step 2: Run the tests to verify one fails**

Run: `python3 -m unittest tests.test_layout -v`
Expected: `test_move_issue_dies_when_the_body_file_is_missing` FAILS — the current guard sits inside the `if old != new` branch, which never fires, so no `SystemExit` is raised.

- [ ] **Step 3: Rewrite `move_issue`**

In `src/trck/templates.py`, replace `move_issue` (lines 164-188) with:

```python
def move_issue(ctx: Ctx, row: Issue, new_status: str) -> None:
    """Set an issue's status and stamp the dates its roles imply. The body file
    does not move — the path encodes id and slug only, so a status change is
    purely an index edit. The existence check stays so a missing body fails here,
    loudly, rather than as a `check` error after the fact."""
    if new_status not in status_names(ctx.cfg):
        die(f"unknown status '{new_status}' (configured: {', '.join(status_names(ctx.cfg))})")
    path = issue_path(ctx, row)
    if not path.exists():
        die(f"file missing for #{row.id}: {path}")
    old_status = row.status
    row.status = new_status

    init = initial_status(ctx.cfg)
    if old_status == init and new_status != init and not row.started:
        row.started = now_utc()
    if is_terminal(ctx.cfg, new_status):
        if not row.closed:
            row.closed = now_utc()
    else:
        row.closed = None
        row.resolution = None
```

Then remove `import shutil` from the `src/trck/templates.py` header **only if** `grep -n "shutil" src/trck/templates.py` shows no other use.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `python3 build.py && python3 -m unittest discover -s tests -v`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
git add src/trck/templates.py tests/test_layout.py trck
git commit -m "move_issue: drop the now-unreachable file move

The path no longer encodes status, so old and new always resolved to the
same file and the shutil.move branch was dead. Status assignment and date
stamping remain; the missing-body guard moves out of the dead branch so it
actually fires."
```

---

### Task 3: Detect a legacy-layout tracker and refuse to operate on it

An engine updated via `trck update` will meet trackers still laid out by status. Without a guard, `check` would report every issue as "markdown file on disk but no index row" plus "in index but no markdown file on disk" — useless noise. Detect the old layout and refuse with one actionable message.

**Files:**
- Modify: `src/trck/config.py:1-5` (import), `src/trck/config.py:117-124` (add `detect_legacy_layout` after `check_status_flags`)
- Modify: `src/trck/index.py:1-7` (imports), `src/trck/index.py:205-207` (`build_ctx_or_die`)
- Modify: `tests/test_layout.py`

**Interfaces:**
- Consumes: `ITEMS_DIR`, `status_names`, `FILENAME_RE`.
- Produces: `detect_legacy_layout(cfg: dict, tracker_dir: Path) -> list[Path]` — sorted paths of issue markdown still sitting in per-status folders; empty list when the tracker is flat. Lives in `config.py` (before `index.py` in MANIFEST) so `build_ctx_or_die` can call it with no ordering question. **It must skip `ITEMS_DIR`**: a tracker is free to configure a status named `items`, and scanning `<tracker>/items/` would then report every correctly-migrated file as unmigrated.
- Produces: `build_ctx_or_die(args, guard_layout: bool = True) -> Ctx`. Task 4's `cmd_migrate_layout` is the only caller that passes `guard_layout=False`.

---

- [ ] **Step 1: Write the failing tests**

Append to `tests/test_layout.py`:

```python
class TestLegacyLayoutGuard(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def legacy(self, tmp):
        """A tracker laid out the old way: one issue file under backlog/."""
        d = make_tracker(tmp, {})
        row = self.t.Issue(id="abc1234", slug="alpha", title="Alpha", kind="task",
                           status="backlog", priority="high")
        ctx = self.t.Ctx(d, self.t.load_config(d))
        old = d / "backlog"
        old.mkdir(parents=True, exist_ok=True)
        (old / self.t.filename(row)).write_text("# Alpha\n")
        self.t.save_index(ctx, [row])
        return d

    def test_detect_legacy_layout_finds_status_folder_files(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp)
            cfg = self.t.load_config(d)
            stale = self.t.detect_legacy_layout(cfg, d)
            self.assertEqual([p.name for p in stale], ["abc1234-alpha.md"])

    def test_detect_legacy_layout_is_empty_for_a_flat_tracker(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            (d / "items").mkdir()
            (d / "items" / "abc1234-alpha.md").write_text("# Alpha\n")
            self.assertEqual(self.t.detect_legacy_layout(self.t.load_config(d), d), [])

    def test_detect_legacy_layout_ignores_non_issue_markdown(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            (d / "done").mkdir()
            (d / "done" / "NOTES.md").write_text("scratch\n")
            self.assertEqual(self.t.detect_legacy_layout(self.t.load_config(d), d), [])

    def test_a_status_named_items_is_legal_and_never_self_detects(self):
        """Statuses no longer name directories, so `items` is an ordinary status
        value. Detection must not mistake the body dir for that status's folder."""
        config = {"statuses": [{"name": "backlog", "role": "initial"},
                               {"name": "items", "role": "active"},
                               {"name": "done", "role": "terminal"}],
                  "aliases": {"start": "items", "done": "done"}}
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, config)
            (d / "items").mkdir()
            (d / "items" / "abc1234-alpha.md").write_text("# Alpha\n")
            self.assertEqual(self.t.detect_legacy_layout(self.t.load_config(d), d), [])

    def test_commands_refuse_a_legacy_layout_tracker(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp)
            with self.assertRaises(SystemExit):
                self.t.build_ctx_or_die(ns(dir=str(d)))

    def test_guard_can_be_bypassed_for_the_migration_verb(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp)
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)), guard_layout=False)
            self.assertEqual(ctx.dir, d)
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `python3 -m unittest tests.test_layout.TestLegacyLayoutGuard -v`
Expected: FAIL — `detect_legacy_layout` does not exist (`AttributeError`), and `build_ctx_or_die` raises `TypeError` on the unexpected `guard_layout` keyword.

- [ ] **Step 3: Add `detect_legacy_layout`**

In `src/trck/config.py`, extend the constants import on line 5 to include `FILENAME_RE` and `ITEMS_DIR`:

```python
from .constants import DEFAULT_UPDATE_REPO, FILENAME_RE, ITEMS_DIR, PR_URL_RE, SELF_PATH, die
```

Add this function immediately after `check_status_flags` (after line 123):

```python
def detect_legacy_layout(cfg: dict, tracker_dir: Path) -> list[Path]:
    """Issue markdown still sitting in per-status folders — the pre-0.23 layout,
    where the containing directory carried the status. Returns the offending paths
    (sorted, one pass per configured status); empty when the tracker is already
    flat. Only well-formed issue filenames count, so a README or scratch note
    parked in an old folder is not mistaken for an unmigrated issue.

    ITEMS_DIR is skipped: statuses no longer name directories, so a tracker may
    legally configure a status called `items`, and scanning the body directory
    would report every correctly-migrated file as unmigrated."""
    out = []
    for name in status_names(cfg):
        if name == ITEMS_DIR:
            continue
        d = Path(tracker_dir) / name
        if not d.is_dir():
            continue
        out.extend(p for p in sorted(d.glob("*.md")) if FILENAME_RE.match(p.name))
    return out
```

- [ ] **Step 4: Guard `build_ctx_or_die`**

In `src/trck/index.py`, extend the config import on line 6 to include `detect_legacy_layout`:

```python
from .config import detect_legacy_layout, load_config, resolve_tracker_dir, resolve_tracker_dir_or_die
```

Replace `build_ctx_or_die` (lines 205-207) with:

```python
def build_ctx_or_die(args, guard_layout: bool = True) -> Ctx:
    """Resolve the tracker and load its config, refusing a tracker still laid out
    by status folder. `guard_layout=False` is for `migrate-layout`, the one verb
    whose whole job is to operate on such a tracker."""
    d = resolve_tracker_dir_or_die(getattr(args, "dir", None))
    ctx = Ctx(d, load_config(d))
    if guard_layout:
        stale = detect_legacy_layout(ctx.cfg, ctx.dir)
        if stale:
            folders = ", ".join(sorted({f"{p.parent.name}/" for p in stale}))
            die(f"legacy status-folder layout: {len(stale)} issue file(s) under "
                f"{folders} — run `trck repo migrate-layout` to move them into "
                f"{ITEMS_DIR}/ (status now lives only in index.jsonl)")
    return ctx
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `python3 build.py && python3 -m unittest discover -s tests -v`
Expected: PASS.

- [ ] **Step 6: Commit**

```bash
git add src/trck/config.py src/trck/index.py tests/test_layout.py trck
git commit -m "detect the legacy status-folder layout and refuse to run

An engine picked up via 'trck update' will meet trackers laid out the old
way. Without a guard, every verb would half-work and 'check' would print
two useless errors per issue. build_ctx_or_die now refuses with one
actionable message pointing at migrate-layout, which opts out of the guard."
```

---

### Task 4: The `migrate-layout` verb

One-shot, idempotent relocation of a legacy tracker into `items/`. Conservative by design: if any file's folder disagrees with its index status, we do not guess which one the user meant — we stop and say so.

**Files:**
- Modify: `src/trck/cmd_maint.py:1-20` (imports), append `cmd_migrate_layout`
- Modify: `src/trck/cli.py` (register the subparser before `install-hook`), and `src/trck/cli.py:1-40` import block
- Modify: `tests/test_layout.py`

**Interfaces:**
- Consumes: `detect_legacy_layout`, `build_ctx_or_die(..., guard_layout=False)` from Task 3; `ITEMS_DIR` from Task 1; `finalize` and `load_index` from the existing engine.
- Produces: `cmd_migrate_layout(args) -> None`, wired to `trck repo migrate-layout [--dry-run]`.

---

- [ ] **Step 1: Write the failing tests**

Append to `tests/test_layout.py`:

```python
class TestMigrateLayout(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def legacy(self, tmp, *specs):
        """Build a legacy-layout tracker. Each spec is (id, slug, status)."""
        d = make_tracker(tmp, {})
        ctx = self.t.Ctx(d, self.t.load_config(d))
        rows = []
        for iid, slug, status in specs:
            row = self.t.Issue(id=iid, slug=slug, title=slug.title(), kind="task",
                               status=status, priority="high")
            folder = d / status
            folder.mkdir(parents=True, exist_ok=True)
            (folder / self.t.filename(row)).write_text(f"# {slug.title()}\n")
            rows.append(row)
        self.t.save_index(ctx, rows)
        return d

    def cap(self, fn, args):
        buf = io.StringIO()
        with redirect_stdout(buf):
            fn(args)
        return buf.getvalue()

    def test_migrate_moves_every_file_into_items(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"),
                                 ("bcd2345", "beta", "done"))
            self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))
            names = sorted(p.name for p in (d / "items").glob("*.md"))
            self.assertEqual(names, ["abc1234-alpha.md", "bcd2345-beta.md"])
            self.assertFalse((d / "backlog").exists())
            self.assertFalse((d / "done").exists())

    def test_migrate_preserves_status_from_the_index(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"),
                                 ("bcd2345", "beta", "done"))
            self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))
            ctx = self.t.build_ctx_or_die(ns(dir=str(d)))
            by_id = {r.id: r.status for r in self.t.load_index(ctx)}
            self.assertEqual(by_id, {"abc1234": "backlog", "bcd2345": "done"})

    def test_check_passes_after_migration(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"))
            self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))
            out = self.cap(self.t.cmd_check, ns(dir=str(d)))
            self.assertIn("OK", out)

    def test_migrate_is_idempotent(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"))
            self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))
            out = self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))
            self.assertIn("nothing to migrate", out)

    def test_dry_run_writes_nothing(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"))
            out = self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=True))
            self.assertIn("abc1234-alpha.md", out)
            self.assertTrue((d / "backlog" / "abc1234-alpha.md").is_file())
            self.assertFalse((d / "items").exists())

    def test_migrate_dies_when_folder_and_index_status_disagree(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"))
            ctx = self.t.Ctx(d, self.t.load_config(d))
            rows = self.t.load_index(ctx)
            rows[0].status = "done"            # index says done, file sits in backlog/
            self.t.save_index(ctx, rows)
            with self.assertRaises(SystemExit):
                self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))
            self.assertTrue((d / "backlog" / "abc1234-alpha.md").is_file())

    def test_migrate_dies_on_a_destination_collision(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"))
            (d / "items").mkdir()
            (d / "items" / "abc1234-alpha.md").write_text("# squatter\n")
            with self.assertRaises(SystemExit):
                self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))

    def test_migrate_leaves_non_issue_files_in_place(self):
        with TemporaryDirectory() as tmp:
            d = self.legacy(tmp, ("abc1234", "alpha", "backlog"))
            (d / "backlog" / "NOTES.md").write_text("scratch\n")
            self.cap(self.t.cmd_migrate_layout, ns(dir=str(d), dry_run=False))
            self.assertTrue((d / "backlog" / "NOTES.md").is_file())  # folder kept
            self.assertTrue((d / "items" / "abc1234-alpha.md").is_file())
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `python3 -m unittest tests.test_layout.TestMigrateLayout -v`
Expected: FAIL — `cmd_migrate_layout` does not exist (`AttributeError`).

- [ ] **Step 3: Implement the verb**

In `src/trck/cmd_maint.py`, extend the imports so this block includes the new names:

```python
from .config import DEFAULT_CONFIG, check_pr, detect_legacy_layout, is_terminal, resolve_alias, resolve_tracker_dir
from .constants import DEFAULT_UPDATE_REPO, ID_ALPHABET, ID_LEN, ITEMS_DIR, SELF_PATH, SINCE_RE, __version__, die
```

Append `cmd_migrate_layout` at the end of the module:

```python
def cmd_migrate_layout(args) -> None:
    """One-shot: relocate every issue body from its per-status folder into
    `items/`. Status stops being encoded in the path and lives only in
    index.jsonl. Idempotent — a flat tracker is a no-op.

    Deliberately conservative about the one ambiguity a legacy tracker can carry:
    if a file's folder disagrees with its index status, the two sources of truth
    have already drifted and only the author knows which is right, so we stop
    rather than silently canonizing one."""
    ctx = build_ctx_or_die(args, guard_layout=False)
    stale = detect_legacy_layout(ctx.cfg, ctx.dir)
    if not stale:
        print(f"migrate-layout: nothing to migrate (already flat in {ITEMS_DIR}/)")
        return

    rows = load_index(ctx)
    by_id = {r.id: r for r in rows}
    dest_dir = ctx.dir / ITEMS_DIR

    drift, collisions, moves = [], [], []
    for p in stale:
        m = FILENAME_RE.match(p.name)
        iid = file_id(m)
        row = by_id.get(iid)
        if row is not None and row.status != p.parent.name:
            drift.append(f"#{iid}: index says '{row.status}', file sits in "
                         f"'{p.parent.name}/'")
            continue
        dest = dest_dir / p.name
        if dest.exists():
            collisions.append(f"#{iid}: {dest} already exists")
            continue
        moves.append((p, dest))

    if drift:
        detail = "\n  ".join(drift)
        die(f"index status and folder disagree for {len(drift)} issue(s); fix the "
            f"index (or move the files) so they agree, then re-run:\n  {detail}")
    if collisions:
        detail = "\n  ".join(collisions)
        die(f"destination already occupied for {len(collisions)} file(s):\n  {detail}")

    if getattr(args, "dry_run", False):
        print(f"migrate-layout: would move {len(moves)} file(s) into {ITEMS_DIR}/")
        for src, dest in moves:
            print(f"  {src.parent.name}/{src.name} -> {ITEMS_DIR}/{dest.name}")
        return

    dest_dir.mkdir(parents=True, exist_ok=True)
    for src, dest in moves:
        shutil.move(str(src), str(dest))

    # Drop the status folders that are now empty. A folder holding anything else
    # (a README, a scratch note) is left alone — rmdir refuses a non-empty dir.
    for folder in {src.parent for src, _ in moves}:
        try:
            folder.rmdir()
        except OSError:
            pass

    finalize(ctx, rows)  # rewrite SUMMARY.md with items/ links, then validate
    print(f"migrate-layout: moved {len(moves)} file(s) into {ITEMS_DIR}/")
```

`cmd_maint.py` already imports `shutil`, `finalize`, `load_index`, and `build_ctx_or_die`. Add `FILENAME_RE` and `file_id` to its imports:

```python
from .constants import DEFAULT_UPDATE_REPO, FILENAME_RE, ID_ALPHABET, ID_LEN, ITEMS_DIR, SELF_PATH, SINCE_RE, __version__, die
from .index import build_ctx, build_ctx_or_die, file_id, issue_path, load_index
```

- [ ] **Step 4: Wire up the CLI**

**Prerequisite:** the maintenance verbs must already be grouped under `trck repo` (tracker issue **#qs4zwzr**), so `migrate-layout` is born in its final namespace rather than renamed in a released CLI. `rsub` below is that group's subparser object — `repo = sub.add_parser("repo"); rsub = repo.add_subparsers(dest="repo_cmd", required=True)`.

In `src/trck/cli.py`, add `cmd_migrate_layout` to the `cmd_maint` import list, then register it alongside the other `repo` verbs:

```python
    ml = rsub.add_parser("migrate-layout",
                         help="one-shot: move issue files from status folders into items/",
                         description="One-shot migration to the flat layout: move every "
                                     "issue body out of its per-status folder into "
                                     "items/, so status lives only in index.jsonl. "
                                     "Idempotent; stops without writing if a file's "
                                     "folder disagrees with its index status.")
    ml.add_argument("--dry-run", action="store_true",
                    help="print what would move, write nothing")
    ml.set_defaults(func=cmd_migrate_layout)
```

- [ ] **Step 5: Run the tests to verify they pass**

Run: `python3 build.py && python3 -m unittest discover -s tests -v`
Expected: PASS.

- [ ] **Step 6: Verify the verb is reachable from the CLI**

Run: `./trck repo migrate-layout --help`
Expected: the help text above, exit 0.

- [ ] **Step 7: Commit**

```bash
git add src/trck/cmd_maint.py src/trck/cli.py tests/test_layout.py trck
git commit -m "add migrate-layout: relocate issue bodies into items/

One-shot and idempotent. Refuses to guess when a file's folder disagrees
with its index status — that drift predates the migration and only the
author knows which side is right. --dry-run prints the planned moves and
writes nothing; empty status folders are removed, folders holding anything
else are left alone."
```

---

### Task 5: Migrate this repo's tracker and the bundled example

Both on-disk trackers in this repo are still laid out by status. Migrate them with the verb we just built — dogfooding it is the acceptance test.

**Files:**
- Modify: `issues/` (122 files move from `backlog/`, `ongoing/`, `done/` into `issues/items/`)
- Modify: `issues/SUMMARY.md` (regenerated with `items/` links)
- Modify: `examples/action-game/` (same treatment)

**Interfaces:**
- Consumes: `trck repo migrate-layout` from Task 4.

---

- [ ] **Step 1: Dry-run this repo's tracker**

Run: `./trck repo migrate-layout --dry-run`
Expected: a list of every issue file with its planned `items/` destination, and no filesystem change. If it dies on status/folder drift instead, resolve the named issues first — they are pre-existing inconsistencies, not migration bugs.

- [ ] **Step 2: Migrate this repo's tracker**

Run: `./trck repo migrate-layout`
Expected: `migrate-layout: moved N file(s) into items/`

- [ ] **Step 3: Verify consistency**

Run: `./trck check`
Expected: `OK — N issues, 0 errors, ...` (a pre-existing warning count is fine).

- [ ] **Step 4: Confirm git recorded pure renames**

Run: `git status --short | head -20`
Expected: `R` entries moving `issues/<status>/*.md` to `issues/items/*.md`, plus modified `issues/SUMMARY.md`. No content changes to any issue body.

- [ ] **Step 5: Migrate the bundled example tracker**

Run: `./trck --dir examples/action-game repo migrate-layout && ./trck --dir examples/action-game check`
Expected: files moved, then `OK`.

- [ ] **Step 6: Regenerate the documentation screenshots**

Run: `python3 docs/gen-screenshots.py`
Expected: exit 0. Then `git diff --stat docs/img/` — if the SVGs changed, the change is cosmetic (the example tracker's rendering), and the regenerated files should be committed.

- [ ] **Step 7: Commit the tracker migration separately from the example**

```bash
git add issues examples docs/img
git commit -m "migrate this repo's tracker and the example to the flat layout

Ran 'trck repo migrate-layout' against issues/ and examples/action-game. Every
issue body moves from its status folder into items/; SUMMARY.md is
regenerated with the new links. No issue body content changed."
```

---

### Task 6: Update the documentation

Three documents describe status as a folder. Each is user-facing and each is now wrong.

**Files:**
- Modify: `README.md:4` and every other folder reference found by grep
- Modify: `src/trck/templates.py` — `CLAUDE_MD_TEMPLATE` (the metadata table row describing status) and `README_TEMPLATE`
- Modify: `CLAUDE.md` (project instructions)
- Modify: `issues/CLAUDE.md` (this repo's copy of the template — regenerate or hand-sync)

**Interfaces:**
- Consumes: nothing. Pure documentation.

---

- [ ] **Step 1: Find every stale reference**

Run:

```bash
grep -rn "folder\|backlog/\|ongoing/\|done/" README.md CLAUDE.md issues/CLAUDE.md src/trck/templates.py \
  | tee /tmp/claude-1000/stale-layout-docs.txt
```

Work through the captured file; the greps below name the ones already known.

- [ ] **Step 2: Fix the README's opening claim**

In `README.md`, replace line 4's clause:

```
your repo. Status is the folder a markdown file sits in; all other metadata lives in
```

with:

```
your repo. Every issue is a markdown file in `items/`; all metadata — status included — lives in
```

Then reword the statuses paragraph near line 85, which currently reads `the folders are named after them`: statuses are an ordered, free-form list that names the values `mv`/`start`/`done` move between and the sections `SUMMARY.md` groups by. They no longer name directories.

- [ ] **Step 3: Fix the `CLAUDE_MD_TEMPLATE` metadata table**

In `src/trck/templates.py`, the status row currently reads:

```
| status | the folder the file is in (configured in `trck.json`) | `trck mv` / `start` / `review` / `done` (moves the file) |
```

Replace with:

```
| status | a value from `trck.json`, stored in `index.jsonl` | `trck mv` / `start` / `review` / `done` |
```

Also update the "walking up to the folder containing trck.json" phrasing only if it refers to the *status* folder — the tracker-discovery sentences are still correct and must stay.

- [ ] **Step 4: Fix the project CLAUDE.md**

In `CLAUDE.md`, the "Tracking work (dogfooding)" section says never to move or rename issue files by hand. Keep that. Add, in the same bullet list:

```markdown
- Issue bodies all live in `issues/items/` — status is **not** encoded in the path; it lives
  only in `index.jsonl`. A `start`/`done` touches the index and `SUMMARY.md`, never the body file.
```

- [ ] **Step 5: Re-sync `issues/CLAUDE.md` with the template**

`issues/CLAUDE.md` is a copy of `CLAUDE_MD_TEMPLATE` written by `trck init`. Apply the same Step 3 edit to it by hand (regenerating would overwrite this repo's local edits).

- [ ] **Step 6: Verify no stale references remain**

Run:

```bash
grep -rn "status is the folder\|the folder the file is in\|moves the file" \
  README.md CLAUDE.md issues/CLAUDE.md src/trck/templates.py
```

Expected: no output.

- [ ] **Step 7: Rebuild and run the suite**

Run: `python3 build.py && python3 -m unittest discover -s tests -v`
Expected: PASS (template text is asserted by some init tests; fix any literal expectations they carry).

- [ ] **Step 8: Commit**

```bash
git add README.md CLAUDE.md issues/CLAUDE.md src/trck/templates.py trck
git commit -m "docs: status is an index field, not a directory

README, the project CLAUDE.md, and the CLAUDE.md template all described
status as the folder a file sits in. Every issue body now lives in items/
and status lives only in index.jsonl."
```

---

### Task 7: Release 0.23.0

**Files:**
- Modify: `src/trck/constants.py:8` (`__version__`)

**Interfaces:**
- Consumes: everything above.

---

- [ ] **Step 1: Bump the version**

In `src/trck/constants.py`, change:

```python
__version__ = "0.22.0"
```

to:

```python
__version__ = "0.23.0"
```

- [ ] **Step 2: Rebuild and verify the engine is in sync**

Run: `python3 build.py && python3 build.py --check`
Expected: `--check` exits 0 with no diff reported.

- [ ] **Step 3: Run the full suite one last time**

Run: `python3 -m unittest discover -s tests -v`
Expected: PASS, zero failures.

- [ ] **Step 4: Verify the tracker and the version**

Run: `./trck check && ./trck version`
Expected: `OK — ...` then `0.23.0`.

- [ ] **Step 5: Commit and tag**

```bash
git add src/trck/constants.py trck
git commit -m "release v0.23.0

Breaking on-disk format: issue bodies move from per-status folders into a
single items/ directory, and status lives only in index.jsonl. Existing
trackers are detected and refused until 'trck repo migrate-layout' runs."
git tag v0.23.0
```

- [ ] **Step 6: Create the GitHub Release**

Per `CLAUDE.md`'s release process, publish a GitHub Release for `v0.23.0` so `trck update` picks it up on the stable channel. **Lead the release notes with the breaking change and the one-line remedy** (`trck repo migrate-layout`) — users updating in place will hit the guard on their next command.

---

## Notes on what deliberately did *not* change

- **`trck set --slug` still renames the body file.** The path encodes id and slug; only status left it. Keeping the slug in the filename is what preserves `ls issues/items/` legibility and `grep` ergonomics, at the cost of a rename on the rare, deliberate slug change.
- **`cmd_which` needed no change.** It matches on basename via `FILENAME_RE` and never looked at the parent directory.
- **`validate` still checks for orphans in both directions** (row without file, file without row). File *existence* remains genuinely duplicated between `index.jsonl` and disk — that is inherent to storing bodies as files, not an artifact of the folder layout.
- **`renumber` still moves files.** The id is in the filename under every layout, so changing an id renames its file.
