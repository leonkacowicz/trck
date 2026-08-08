# summary: partition open work — every non-terminal row exactly once

## Summary
`SUMMARY.md` has two organizing axes: a **Hierarchies** section holding epics with their children, and
**Backlog / In-progress / In-review / Done** sections holding everything else. So where an issue
appears depends on whether it happens to have a parent — a rule a reader has to know before they know
where to look.

Worse, the two axes are filters rather than a partition, so a row can match neither:

- **A non-terminal child under a terminal epic.** Reachable only via a manually pinned status, since a
  parent's status is derived — but reachable.
- **A row whose `parent` points at an id that is not in the index.** `check` flags it, but `summary`
  is generated on every mutating verb and would be showing an incomplete picture until someone runs
  `check`.

Neither happens on this repo's tracker today (verified: 0 rows fall through, no pinned statuses, no
dangling parents). An invisible open issue is still the worst thing a summary can do, and the fix is
to make the sections a partition with a leftover bucket rather than three independent filters.

Also: the current rule tests `status == "done"` rather than asking whether the status is terminal.
Same answer today, since `done` is the only terminal status, but `config::is_terminal` exists and
`list` already uses it via `is_settled`. A second copy of the rule in `summary.rs` is how the two
views start disagreeing.

This is the ordering rule [[summary-hide-settled-work-and-collapse-completed-epics]] implements, so it
lands first.

## Acceptance criteria
- [ ] Every non-terminal row appears exactly once in the open sections — asserted by a test that walks
      the generated file and compares its id set against the index, not by inspection.
- [ ] A leftover bucket catches anything the structural rules miss (dangling parent, pinned status
      under a terminal parent) rather than dropping it.
- [ ] Terminal-ness comes from `config::is_terminal`, not from a literal `"done"`.
- [ ] A nested epic appears once, not once as its own heading and once as a child row.
- [ ] Conformance fixture: a tracker with a non-terminal child under a manually pinned terminal
      parent, and one with a dangling `parent`.

## Notes
- `src/summary.rs`: `parents` (id-sorted) and the per-status `items` (priority, then id).
- `list`'s rule is `is_settled` in `src/query/list.rs` — terminal **and** under a terminal parent, so
  a done child stays visible under an open epic as progress context. This issue does not have to
  adopt that rule, but it has to *pick* one and share the predicate rather than reimplement it.
