# graph: demand cone + priority-count vector (effective blocking, reversed)

## Summary
Add the derived read view the ranking needs to `Graph` (`src/trck/graph.py`), beside
`is_blocked`/`is_ready`: the set of issues that transitively wait on an issue, and the
per-priority counts over it.

## Acceptance criteria
- [ ] `demand_cone(r)` — `r` plus every non-terminal issue that transitively demands it
- [ ] follows authored dependencies under the lifting rule (`subtree(a)` demands `subtree(b)`
      for each authored `a -> b`) **and** containment (a node is demanded by its parent)
- [ ] terminal issues are excluded from the cone and do not conduct demand through them
- [ ] `demand_vector(r)` — counts per configured priority, config order, unknown last
- [ ] `demand_source(r)` — the highest-priority other cone member when it outranks `r`,
      else `None`; deterministic tie-break among equals
- [ ] memoized per `Graph`, so ranking the whole ready set is one pass, not one walk per row
- [ ] tests: dependency chain, inheritance down a dependent's subtree, pure containment,
      terminal pruning, diamond (no double counting), cousin isolation

## Notes
The reverse of `_eff_reach`. Building it as reverse adjacency and closing over it keeps
the walk linear in the reachable set:

    demands(m) = {parent(m)} ∪ ⋃ { subtree(a) : authored a -> b, m ∈ subtree(b) }

The `subtree(a)` term matters: a dependent's *children* inherit the edge, so an urgent
child of a medium dependent must reach back to the blocker.

Non-terminal restriction applies to both ends, so a cone never contains settled work.

`priority_rank` lives in `render.py`, which the amalgamation emits *after* `graph.py` —
fine inside a method body (one namespace at call time), but add the sibling import so
editors resolve it.
