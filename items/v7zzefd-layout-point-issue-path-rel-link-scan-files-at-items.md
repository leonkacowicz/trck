# layout: point issue_path/rel_link/scan_files at items/

## Summary
The core flip. After this, new issues land in `<tracker>/items/`, `scan_files` reads them from
there, and the status-vs-folder drift check is gone. `move_issue`'s `shutil.move` becomes dead
code (old and new paths are now equal, so the `if` never fires) — #zk5k59n removes it.

## Implementation

**`src/trck/constants.py`** — add after `FILENAME_RE` (line 13):
```python
ITEMS_DIR = "items"  # the one directory holding every issue body; status lives in index.jsonl
```

**`src/trck/index.py`** — import `ITEMS_DIR`, then replace `rel_link`/`issue_path` (lines 225-230):
```python
def rel_link(row: Issue) -> str:
    return f"{ITEMS_DIR}/{filename(row)}"


def issue_path(ctx: Ctx, row: Issue) -> Path:
    return ctx.dir / ITEMS_DIR / filename(row)
```

**`src/trck/scan.py`** — replace `scan_files` (lines 10-25). The return shape drops the leading
folder element: `id -> (slug, filename)`. `validate` is its only caller.
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

**`src/trck/scan.py`** — in `validate`, replace lines 48-50:
```python
        folder, slug, fname = files[iid]
        if r.status != folder:
            errors.append(f"#{iid} index status '{r.status}' != folder '{folder}'")
```
with just `slug, fname = files[iid]`. Leave the slug and filename checks that follow — they
still guard the one piece of metadata the path does encode.

**`src/trck/graph.py`** — `_existing_ids` (lines 541-552) globs the items dir instead of every
status folder; drop the now-unused `status_names` from the module's config import.
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

**Tests** — new `tests/test_layout.py::TestFlatLayout` covering: `new` writes into `items/`; a
status change moves no file; `rel_link` and the `SUMMARY.md` links point into `items/`; a slug
change still renames within `items/`; `scan_files` returns the 2-tuple. Then fix the two
layout-coupled existing tests: `tests/test_paths.py:122` (`issues/backlog/` → `issues/items/`)
and delete `tests/test_validate.py::test_status_folder_mismatch_is_error` — the drift it
asserted is now unrepresentable.

## Acceptance criteria
- [ ] `ITEMS_DIR` defined in `constants.py`; the literal `"items"` appears nowhere else in the engine
- [ ] `issue_path` resolves to `<tracker>/items/<id>-<slug>.md`; `rel_link` to `items/<file>`
- [ ] `scan_files` walks one directory and returns `id -> (slug, filename)`
- [ ] The index-status-vs-folder error is gone from `validate`
- [ ] `_existing_ids` globs `items/`
- [ ] Full suite green after `python3 build.py`

## Notes
Step-by-step TDD cycle with the full test code: `docs/plans/2026-07-30-flat-items-layout.md`
(Task 1).
