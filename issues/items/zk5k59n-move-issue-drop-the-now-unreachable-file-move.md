# move_issue: drop the now-unreachable file move

## Summary
With the path no longer status-derived (#v7zzefd), `move_issue`'s `old.resolve() != new.resolve()`
is always false and the `shutil.move` inside it is unreachable. Remove the dead branch — and move
the missing-body guard *out* of it, so the guard actually fires.

## Implementation

**`src/trck/templates.py`** — replace `move_issue` (lines 164-188):
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

Then drop `import shutil` from the module header **only if** `grep -n "shutil"
src/trck/templates.py` shows no other use.

Callers (`cmd_mv`, `normalize_statuses`) are unchanged — the signature is the same.

**Tests** — add to `tests/test_layout.py::TestFlatLayout`: `move_issue` dies when the body file
is missing (currently it cannot, because the guard sits in the dead branch), and `move_issue`
stamps `started`/`closed` while leaving the body file's mtime untouched.

## Acceptance criteria
- [ ] `move_issue` contains no `shutil.move`
- [ ] A missing body file makes `move_issue` exit nonzero
- [ ] Date stamping (`started`, `closed`, resolution clearing) is unchanged
- [ ] Full suite green

## Notes
Step-by-step TDD cycle with the full test code: `docs/plans/2026-07-30-flat-items-layout.md`
(Task 2).
