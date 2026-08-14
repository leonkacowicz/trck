# writes: replay stacked pending commits, and surface an op that no longer applies

## Summary
Replay is trivial at depth one — the verb just ran. It is not trivial once commits stack:
three issues filed offline, the remote moved, and three operations have to be replayed that are
no longer in memory. That is what the trailer is for.

And replay can legitimately fail: an op referencing an issue someone else closed differently is a
real conflict, and belongs in front of a human rather than being resolved silently.

## Acceptance criteria
- [ ] Pending commits replay in order against the refetched ref and converge at depth greater than one.
- [ ] A replayed op that no longer applies stops the sequence, reports the op and the reason, and leaves the local ref where it was — no partial application.
- [ ] Replay is deterministic: same pending commits and same remote tree give the same result.
- [ ] Covered by the #A4 harness with two writers and a stack of at least three pending commits.

## Notes
Given the trailer, the `trck-index` merge driver becomes vestigial on this path. Without it, stacked commits would have to fall back to tree merging — which is the dependency this epic removes, so the trailer is not optional.
