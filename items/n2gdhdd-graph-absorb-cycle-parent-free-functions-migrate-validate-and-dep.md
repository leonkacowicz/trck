# Graph: absorb cycle/parent free functions; migrate validate and dep

## Summary

Phase 1b of the Graph epic (#032) — decision (b) from the spec. Give the dependency
graph one home: fold the standalone `find_dep_cycles`, `dep_would_cycle`, and
`parent_ids` into `Graph` as `cycles()`, `would_cycle(src, dep)`, and `is_leaf`, then
delete the free functions. `validate` and `cmd_dep` build a `Graph` and call the methods.

The larger, test-heavier step in the epic — sequenced last among the Graph phases so it
lands independently.

## Acceptance criteria
- [ ] `find_dep_cycles`, `dep_would_cycle`, `parent_ids` removed; equivalent `Graph`
      methods (`cycles()`, `would_cycle()`, `is_leaf` / `_parents`) in place.
- [ ] `validate` uses `g.cycles()`; `cmd_dep` uses `g.would_cycle()`.
- [ ] `normalize_points`' use of `parent_ids` is rerouted through the Graph (or kept
      correct if it stays on the raw rows — decide and note).
- [ ] Tests assert parity with the old free functions (same cycles, same would-cycle verdicts).
- [ ] `trck check` passes; all tests green.

## Notes

This is the seam before #031: Phase 2 (#036) depends on this completing.
