# ready/next: scope to a subtree (what can I pick up on this parent now)

## Summary

`trck ready` / `trck next` consider the whole tracker. Accept an optional issue id to scope
them to that issue's subtree: `trck ready <epic>` = the actionable leaves under that epic,
`trck next <epic>` = the single best pick within it.

This is the *frontier* half of "what is left to finish this epic" — the smallest useful
answer, and probably the one reached for most often day to day. #dj5b42j answers the
structural half (the whole remaining graph, ordered); this one answers "what do I do right
now". Deliberately separate: no graph work, no inferred edges, no renderer changes.

Blocking must stay **effective**, not local — a leaf inside the subtree blocked by an
authored dependency on something *outside* it is still blocked, and must not be listed.

## Acceptance criteria
- [ ] `trck ready <id>` lists actionable leaves within that subtree only.
- [ ] `trck next <id>` picks the single best one, same ordering rules as unscoped `next`.
- [ ] With no id, behaviour is unchanged.
- [ ] A leaf blocked by an out-of-subtree dependency (directly or inherited) is excluded.
- [ ] An id resolving to a leaf yields just that leaf when it is ready, nothing when not.

## Notes

- `Graph.subtree` already gives the node set; `is_ready` (`trck:684`) already composes
  terminal / leaf / `is_blocked` and reads `lifted_deps`, so effective blocking comes for
  free provided the filter is applied *after* readiness rather than restricting the graph.
- Argument shape should match `list [ID]`, which already takes an optional positional id to
  root the view — same mental model, same resolution via `resolve_ref` (prefix / legacy id).
