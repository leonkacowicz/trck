# version/update: suppress no-tracker stderr noise when run outside a tracker

## Summary
Both `trck version` and `trck update`, when run outside any tracker, print
`error: no tracker found here` to stderr (from `die` inside `find_tracker`) before
succeeding — `cmd_version` and `_update_repo` catch the `SystemExit` but the message was
already written. Add an optional (non-dying) tracker resolution and use it in both so they
resolve the tracker dir silently.

## Acceptance criteria
- [ ] `trck version` with no tracker prints only the version; stderr is clean
- [ ] `trck update` outside a tracker falls back to the default repo without stderr noise
- [ ] `build_ctx` / `resolve_tracker_dir` / `find_tracker` support an optional (returns-None)
      mode; the default (required) behavior is unchanged for all other verbs

## Notes
Shared root cause across `version` and `update` (folded together). Surfaced live during the
v0.1.1 self-update verification.
