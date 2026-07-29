# ready/next: rank by the demand vector instead of declared priority

## Summary
Swap `cmd_ready`'s sort key from `(priority_rank, -points, id)` to the demand vector
from #5yjce3w, so the top pick is the work that unblocks the hottest issue rather than
the work that merely *is* the hottest issue.

## Acceptance criteria
- [ ] key is the negated per-priority counts, then `-points`, then `id`
- [ ] `next` (and `ready --next`) picks the top of that order
- [ ] subtree-scoped `ready ID` ranks over the full graph, then filters — scoping must not
      change the ranking of what it shows
- [ ] tests: blocker of an urgent issue outranks a higher-priority row that blocks nothing;
      equal max priority breaks by how many issues at that level are blocked; existing
      `-points`/`id` tie-breaks still apply

## Notes
Cones are never nested within the ready set — if `X` blocks `Y`, `Y` is not ready — so
counts never inflate each other across the rows being compared. Two ready rows blocking
the *same* urgent issue tie on that slot, which is correct: both have to happen.
