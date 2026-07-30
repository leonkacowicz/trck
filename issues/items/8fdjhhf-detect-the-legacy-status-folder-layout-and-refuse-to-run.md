# detect the legacy status-folder layout and refuse to run

## Summary
`trck update` replaces the engine in place, so a v0.23+ engine will meet trackers still laid out
by status folder. Without a guard, `check` reports every issue twice — once as "markdown file on
disk but no index row", once as "in index but no markdown file on disk" — which is noise, not a
diagnosis. Detect the old layout and refuse with one actionable message.

## Implementation

**`src/trck/config.py`** — add after `check_status_flags` (after line 123). It lives here, not in
`scan.py`, because `config` precedes `index` in `build.py::MANIFEST`, so `build_ctx_or_die` can
call it with no module-ordering question.
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
Extend the constants import to bring in `FILENAME_RE` and `ITEMS_DIR`.

**`src/trck/index.py`** — replace `build_ctx_or_die` (lines 205-207):
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

**Tests** — new `tests/test_layout.py::TestLegacyLayoutGuard`: detection finds status-folder
files; is empty for a flat tracker; ignores non-issue markdown (a `NOTES.md` parked in an old
folder); every command refuses a legacy tracker; `guard_layout=False` bypasses it. Plus the
regression that matters — **a status named `items` is legal and never self-detects**.

## Acceptance criteria
- [ ] `detect_legacy_layout(cfg, tracker_dir)` returns sorted offending paths, `[]` when flat
- [ ] It skips `ITEMS_DIR`, so a status named `items` never self-detects
- [ ] Only filenames matching `FILENAME_RE` count; scratch markdown is ignored
- [ ] `build_ctx_or_die` refuses a legacy tracker with a message naming the folders and the remedy
- [ ] `guard_layout=False` bypasses the guard
- [ ] Full suite green

## Notes
The error message names `trck repo migrate-layout` — the grouped namespace decided in #qs4zwzr.

Step-by-step TDD cycle with the full test code: `docs/plans/2026-07-30-flat-items-layout.md`
(Task 3).
