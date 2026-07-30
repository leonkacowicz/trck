# Crash instead of clean error when no tracker is found

## Summary
`resolve_tracker_dir_or_die` raises an unhandled `TypeError` (full traceback) instead
of a clean `error: …` message when no tracker is discoverable **and** no `--dir` was
given. This is the path nearly every verb takes via `build_ctx_or_die`, so running
e.g. `trck list` or `trck new …` from a directory with no `trck.json` anywhere up the
tree dumps a Python stack trace at the user.

The cause (`trck:171-176`):

```python
def resolve_tracker_dir_or_die(dir_opt, env=None) -> Path:
    path = resolve_tracker_dir(dir_opt, env=env, required=False)
    if not path:
        p = Path(dir_opt).resolve()   # dir_opt is None here -> Path(None) -> TypeError
        die(f"{p} is not a tracker (no trck.json)")
    return path
```

When `dir_opt` is `None`, `resolve_tracker_dir(..., required=False)` returns `None`
(via `find_tracker(required=False)`), and the recovery branch then calls `Path(None)`,
which raises `TypeError` before `die()` is ever reached. The helper silently assumes
`dir_opt` is truthy.

## Acceptance criteria
- [ ] Running a verb with no tracker found and no `--dir` exits non-zero with a clean
      `error: …` message (e.g. "no tracker found here; run `trck init`"), no traceback.
- [ ] The existing clean message for an explicit but invalid `--dir`/`$TRCK_DIR` is
      preserved (still names the offending path).
- [ ] A regression test covers the `dir_opt is None` + no-tracker case (assert the
      message, assert no exception escapes).

## Notes
- Mirror the message `find_tracker` already uses for the required case
  ("no tracker found here; run `trck init`") so behavior is consistent whether the
  walk-up dies internally or the `_or_die` wrapper does.
- Only branch into the `Path(dir_opt)` message when `dir_opt` is actually set.
