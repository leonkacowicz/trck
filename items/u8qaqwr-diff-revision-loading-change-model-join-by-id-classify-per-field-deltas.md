# diff: revision loading + change model (join by id, classify per-field deltas)

## Summary
The foundation every rendering layer sits on: get two sets of rows, join them, and produce a typed
description of what changed. No output formatting here beyond whatever the tests need.

Two halves:

1. **Loading.** Resolve a revision spec to a row list — `git show <rev>:<tracker>/index.jsonl`
   parsed with the existing index reader; the "current" side is the working tree. Handle the
   tracker dir not existing at that revision (everything is `added`) and `<a>..<b>` two-sided specs.
2. **The change model.** Join old/new by id and classify:
   - `added` / `removed` (a row present on one side only),
   - per-field deltas for scalars (status, priority, points, parent, kind, title, slug, spec, pr,
     and custom fields),
   - set deltas for `labels` and `depends_on` (`+x −y`),
   - **direction** for a status change: forward / backward / lateral, computed from the configured
     status order in `trck.json` — a `done → ongoing` reopen must be distinguishable from a
     `backlog → ongoing` start, since every layer renders the two differently.

## Acceptance criteria
- [ ] A function returns, for a pair of revisions, a list of per-issue change records plus the two
      row sets (renderers need the full rows for titles, icons, and rollups).
- [ ] Field classification is data-driven — no hard-coded status or priority names; direction comes
      from the config's status order, as with the `status_*` helpers.
- [ ] Timestamp fields (`created`/`started`/`closed`) are recorded but not reported as ordinary
      field edits — they are the evidence for a status change, not a separate change.
- [ ] Tests cover: added, removed, status forward/backward, multi-field edit, label/dep set deltas,
      tracker absent at the old revision, and a revision spec git can't resolve (clean error).

## Notes
- Everything else in the epic depends on this; keep the record shape renderer-agnostic.
- Rows at an old revision may use a vocabulary the current `trck.json` no longer has (a renamed
  status). Don't crash — treat an unknown status as lateral/unordered and render it verbatim.
- Reuse `merge.py`'s `_by_id` / `_tuple_of` if they fit; that module already compares rows by id.
