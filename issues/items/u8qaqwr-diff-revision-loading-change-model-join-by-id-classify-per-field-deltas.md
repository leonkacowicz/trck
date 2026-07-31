# diff: change model — join snapshots by id, classify per-field deltas

## Summary
The foundation every rendering layer sits on: given two **snapshots** (#q9cq65c), join them by id
and produce a typed description of what changed. Pure — no I/O, no revisions, no `subprocess`, no
output formatting beyond what the tests need.

Classification:
- `added` / `removed` — a row present on one side only;
- per-field deltas for scalars (status, priority, points, parent, kind, title, slug, spec, pr, and
  custom fields);
- set deltas for `labels` and `depends_on` (`+x −y`);
- **direction** for a status change: forward / backward / lateral, computed from the configured
  status order in `trck.json` — a `done → ongoing` reopen must be distinguishable from a
  `backlog → ongoing` start, since every layer renders the two differently.

## Acceptance criteria
- [x] A function takes two snapshots and returns per-issue change records plus both row sets
      (renderers need full rows for titles, icons, and rollups).
- [x] Field classification is data-driven — no hard-coded status or priority names; direction comes
      from the config's status order, as with the `status_*` helpers.
- [x] Timestamp fields (`created`/`started`/`closed`) are recorded but not reported as ordinary field
      edits — they are the evidence for a status change, not a separate change.
- [x] Nothing in this module imports `subprocess` or knows what a revision is.
- [x] Tests cover: added, removed, status forward/backward, multi-field edit, label/dep set deltas,
      and an empty old snapshot (everything reads as added) — all from fixture snapshots, no git.

## Notes
- Every renderer depends on this; keep the record shape renderer-agnostic.
- Rows from an old snapshot may use a vocabulary the current `trck.json` no longer has (a renamed
  status). Don't crash — treat an unknown status as lateral/unordered and render it verbatim.
- Reuse `merge.py`'s `_by_id` / `_tuple_of` if they fit; that module already compares rows by id for
  the 3-way merge driver, and `diff` is the read side of the same coin.
