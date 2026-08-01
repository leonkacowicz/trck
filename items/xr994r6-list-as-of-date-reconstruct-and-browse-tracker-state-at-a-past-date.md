# list --as-of DATE: reconstruct and browse tracker state at a past date

## Summary
Date-as-navigation rather than date-as-data: run any read verb against the tracker as it stood at
a past point. `trck list --as-of 2026-06-01`, `trck ready --as-of last-release`, `trck deps --as-of
v0.23.0` — the same views, a different snapshot.

Most of this already exists. `gitsrc.py` loads an `index.jsonl` at an arbitrary ref, `diff`'s
`--from/--to` already accept revision specs, and every read verb builds its view from a loaded row
list. `--as-of` is largely a matter of resolving a date to the last commit at or before it and
routing the loader through the existing source seam.

The nice property is that it answers the "what did we look like then" questions without any schema
change — and it composes with everything, including the date filters in [[v5wvabj]].

## Acceptance criteria
- [ ] `--as-of DATE|REF` on the read verbs, resolving a date to the last commit at or before it.
- [ ] Bodies resolve at that revision too (or the limitation is stated — `show` is the case that
      cares).
- [ ] A clear error outside a git repo, or when the date predates the tracker.
- [ ] Output states which revision it resolved to, so the result is reproducible.
- [ ] Tests use a real git fixture, in the style of #2ry5d58.

## Notes
- Needs the direction from [[gybeetp]]: if that lands on git reconstruction, this is a small
  extension of the same machinery; if it lands on an event log, this needs re-scoping (an event log
  gives transitions, not whole past snapshots — reconstructing a snapshot means replaying it).
- Seams: `gitsrc.py`, the `--from/--to` handling in `diff` (#q9cq65c, #wtmfdhr).
- Read-only by construction — `--as-of` must never be accepted by a mutating verb.
