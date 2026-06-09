# Graph: migrate ready/next and deps to the Graph view

## Summary

Phase 1 of the Graph epic (#032). Rewire the simple read commands that #031 never
touches onto the `Graph` from #033, proving the API on real callers with zero overlap.
Output must stay byte-identical.

- `cmd_ready` / `cmd_next`: the `by_id` / `parents` setup and the `is_ready` closure
  collapse to `r for r in g.rows if g.is_ready(r)`.
- `cmd_deps`: `by_id`, the hand-rolled `reverse` map, and the `requires` / `blocks`
  closures vanish; `walk_tree`'s `children_fn` becomes `g.requires_of` / `g.dependents_of`.

## Acceptance criteria
- [ ] `cmd_ready` / `cmd_next` use `load_graph` + `g.is_ready`; no local `by_id`/`parents`.
- [ ] `cmd_deps` uses `g.requires_of` / `g.dependents_of`; no local `by_id`/`reverse`/closures.
- [ ] Output of `ready`, `next`, and `deps` is unchanged (existing tests stay green).
- [ ] `trck check` passes.

## Notes

`list` and `tree` are intentionally left alone here — #031 migrates them on top of the
Graph so they are written once.
