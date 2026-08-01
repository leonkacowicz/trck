# list/ready: overdue and due-soon markers on rows

## Summary
A stored deadline is useless if you have to ask for it. Once `due` is a real field, mark rows that
are overdue or coming up, in the shared row renderer alongside the existing `needs #NNN` /
`blocks #NNN` and `↑<priority>(#id)` annotations — an in-place marker, not a new view.

- **overdue** — non-terminal and `due` < today. Marked prominently (the one place colour is
  warranted).
- **due soon** — non-terminal and `due` within a configurable window. Marked dimly.
- A terminal issue is never marked, whatever its `due` says.

Plus the obvious filter: `--overdue` on `list` and `ready`, so "what's late" is one command.

The window comes from `trck.json`, like the staleness threshold in [[teawzv6]] — same reasoning,
and the two should share a config section rather than inventing separate keys.

## Acceptance criteria
- [ ] Overdue and due-soon markers render on `list` and `ready` rows by default.
- [ ] Terminal rows are never marked.
- [ ] `--overdue` filters to late, non-terminal work.
- [ ] The due-soon window is configurable in `trck.json` with a default.
- [ ] `SUMMARY.md` gains no now-dependent value.
- [ ] Tests pin both markers, the boundary days, and the terminal-row exemption.

## Notes
- Needs [[x6argpr]].
- Coordinate the config shape with [[teawzv6]] — one `dates`/`thresholds` block, not two.
