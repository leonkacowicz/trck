# mv/done: guard closing a parent with open descendants (--recurse to cascade)

## Summary
Moving an issue to a terminal status while it still has non-terminal descendants
produces a misleading state: the parent reads "done" but real work under it is open.
Add a guard, plus a `--recurse`/`--recursive` flag that cascades the terminal
status + resolution to all still-open descendants in one move.

This is **independent of the points rollup work**. The rollup change affects only the
*percentage math*; this issue is about `mv`/`done` *semantics*. Keep them in separate
commits.

Filed out of a design discussion — see Notes for the open questions to resolve before
implementing (block vs. warn is not yet decided).

## Acceptance criteria
- [ ] Moving an issue to a terminal status while it has non-terminal descendants is
      handled deliberately (see open question: hard block vs. warning).
- [ ] A `--recurse` (alias `--recursive`) flag cascades the same terminal status and
      `--resolution` to every non-terminal descendant in the subtree, setting their
      `closed` dates and moving their files.
- [ ] `--recurse` prints the full list of issues it is about to close before doing so.
- [ ] Logic keys on "any terminal status" (vocabulary-agnostic), enforced in `mv` — not
      on the literal word "done". The `done` alias inherits the behavior for free.
- [ ] Tests cover: guard triggers on a parent with open descendants; `--recurse` closes
      the whole subtree with the given resolution; deep (grandchild) descendants are
      reached; an already-all-terminal subtree closes without needing `--recurse`.

## Notes
Design context (from discussion):

- **Why it's only ergonomic, not integrity-critical:** with leaves-only deep rollup, a
  parent's own status does not feed the rollup percentage (only leaves do). So the
  rollup already exposes the inconsistency; this guard keeps the parent's *status flag*
  honest, it does not fix the math.
- **Open question — block vs. warn:** a hard block is the strict choice; a warning that
  still allows the move is lower-friction and may suffice since the rollup already shows
  the truth. Leaning **warn-by-default + `--recurse` to cascade**, but a hard block is
  defensible. Decide before implementing.
- **Cascade is heavy/semi-destructive:** `done X --resolution wontfix --recurse` would
  close descendants that were e.g. `ongoing` as wontfix. Hence the requirement to list
  affected issues first.
- **Asymmetry (out of scope):** there is no cascade-reopen counterpart; reopening a
  parent will not reopen children. Deliberate omission.
- Touchpoints: `cmd_mv` / `move_issue` / the `done` alias path, argparse for `mv` and
  `done`, and the children/descendant walk (reuse a subtree walk with a cycle guard).
