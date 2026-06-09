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
- [x] `list` marks blocked rows and names their open blockers (e.g. `needs #007`).
- [x] Annotation lists only *non-terminal* blockers; a done blocker is omitted.
- [x] A row whose every dependency is terminal shows no blocked marker (block cleared).
- [x] "blocked" uses the same `is_blocked` definition as `ready`/`next` (no drift) —
      `needs` keys off the very same non-terminal-dependency test.
- [x] Reverse side surfaced too: `blocks #NNN` lists the issues depending on the row,
      shown only while the row is non-terminal (a done task blocks nothing → cleared).
- [x] Annotations are styled dim (`paint(..., "dim")`) and degrade to plain text with
      color disabled.
- [x] `ready`/`next` unchanged: `print_rows` gained an opt-in `annotate` callback that
      only `cmd_list` passes; `ready`/`next` call it without one.
- [x] Tests cover: blocked row's `needs`, omission of a done blocker, both-sides clearing,
      the `blocks` reverse annotation, no-color rendering, and ready staying terse.

## Notes
Relevant code: `print_rows` (`trck`), `is_blocked` (the readiness inverse shared by
`list --blocked` and `ready`/`next`), and `cmd_deps` for the requires/blocks derivation
to reuse. The reverse edge (who depends on a row) is built in `cmd_deps` by scanning
`depends_on` across all rows — the same reverse map can feed the `blocks #…` annotation.

Consider interaction with `#024` (`--json` output): the structured output may want a
`blocked_by` / `blocks` field, but this issue is about the human list rendering.

**Decisions made while implementing:**
- An edge X→Y (Y depends on X) renders on *both* ends: `needs #X` on Y and `blocks #Y`
  on X. `needs` is gated on the blocker X being non-terminal (so it mirrors `is_blocked`
  exactly); `blocks` is gated on the row X itself being non-terminal (a done task blocks
  nothing). So finishing a blocker clears the note symmetrically on both rows.
- `blocks` lists dependents regardless of *their* status (it's the literal reverse graph).
- Scoped to `list` only. `ready`/`next` stay terse via the opt-in `annotate` param on the
  shared `print_rows`; their rows are unblocked by construction so `needs` is moot anyway.
- Reverse map is built once in `cmd_list` from the *full* index (before filtering), so a
  blocker hidden by a `--status`/`--parent` filter still drives the annotation correctly.
- New helper `block_annotations(ctx, r, by_id, reverse)` lives by `node_label`. Help text
  (`list` description) and README "Dependencies" section updated.
