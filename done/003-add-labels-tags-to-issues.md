# Add labels/tags to issues

## Summary
Add a `labels` array field to issues, with verbs to add/remove labels and
filter by them, and **remove the `milestone` field entirely** in favor of
labels. Builds on preserve-unknown-keys for forward-compat.

Milestone is today a half-finished single-purpose grouping slot: free-text
(not config-validated), not filterable via `list`, only surfaced in
`SUMMARY.md` for children of an epic (never for standalone issues), and its
SUMMARY "overview" is a per-child status ribbon, not a real aggregation. A
general `labels` mechanism subsumes it, so milestone is dropped rather than
kept as a parallel axis.

## Acceptance criteria
- [x] `trck label NNN --add X --remove Y`
- [x] `trck list --label X`
- [x] labels shown in `show`/`summary` (also `list`/`tree`)
- [x] `milestone` field removed: dropped from `new`/`set` flags, from the
      index field set, from `list` display, and from all `SUMMARY.md`
      rendering (sort key, overview strip, per-child prefix)
- [x] migration: existing `milestone: "X"` values are converted to a label
      `"X"` (handled on load, persisted by `normalize`), so no data is lost
- [x] docs updated (README, scaffolded `CLAUDE.md`, top-level `--help`
      epilog which still references milestone)

## Notes
- Labels are a flat, multi-valued, unordered set of strings per issue —
  intentionally simpler than milestone (single-valued + ordered + rollup).
  Those milestone-specific behaviors are deliberately *not* carried over; if a
  release/phase rollup is wanted later, it can be a separate feature.
- Migration naming: convert `milestone:"v1.0"` to the plain label `"v1.0"`.
  Namespacing as `milestone:v1.0` was considered but rejected — dropping the
  concept means it's just a label.
- Decision recorded after the v0.2.0 milestone-vs-labels discussion.
