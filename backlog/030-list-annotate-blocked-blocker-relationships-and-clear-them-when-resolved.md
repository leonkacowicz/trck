# list: annotate blocked/blocker relationships and clear them when resolved

## Summary
Today the dependency (blocking) graph is only visible per-issue via `trck deps NNN`.
The list views (`list`, `ready`, `next` — all share `print_rows`) show no indication
of blocking at all: a blocked issue looks identical to an unblocked one, and there is
no hint of *which* issue is the blocker or *which* issues a row blocks.

`list --blocked` already filters to blocked rows, but even then the output gives no
clue about the relationship or why the row is blocked.

Make the blocking graph legible directly in the list output:

- Mark a row that is **blocked** (has at least one non-terminal dependency) — e.g. a
  distinct icon/tag and a `needs #NNN[,#MMM…]` annotation naming the open blockers.
- Optionally mark a row that **blocks** others — e.g. `blocks #NNN…` — so blockers are
  visible without running `deps` on each candidate.
- When a blocker reaches a terminal status, the block is **cleared**: the dependent row
  must no longer show as blocked and must drop the resolved id from its `needs` list.
  (This falls out of keying "blocked" off non-terminal deps via `is_blocked`, which
  `ready`/`next` already do — keep the two definitions consistent.)

Annotations should be dim/secondary so the existing `{icon} #id status priority title`
line stays readable, and should respect the no-color path.

## Acceptance criteria
- [ ] `list` marks blocked rows and names their open blockers (e.g. `needs #007`).
- [ ] Annotation lists only *non-terminal* blockers; a done blocker is omitted.
- [ ] A row whose every dependency is terminal shows no blocked marker (block cleared).
- [ ] "blocked" uses the same `is_blocked` definition as `ready`/`next` (no drift).
- [ ] Decide and implement whether to also surface the reverse side (`blocks #NNN`);
      if shown, it lists dependents regardless of their status.
- [ ] Annotations are styled dim and degrade cleanly with color disabled.
- [ ] `ready`/`next` output is unaffected by definition (their rows are never blocked),
      but the shared `print_rows` change must not break them.
- [ ] Tests cover: a blocked row's annotation, omission of a done blocker, full clearing
      when all deps are terminal, and the no-color rendering.

## Notes
Relevant code: `print_rows` (`trck`), `is_blocked` (the readiness inverse shared by
`list --blocked` and `ready`/`next`), and `cmd_deps` for the requires/blocks derivation
to reuse. The reverse edge (who depends on a row) is built in `cmd_deps` by scanning
`depends_on` across all rows — the same reverse map can feed the `blocks #…` annotation.

Consider interaction with `#024` (`--json` output): the structured output may want a
`blocked_by` / `blocks` field, but this issue is about the human list rendering.
