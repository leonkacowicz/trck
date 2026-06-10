# Deduplicate parent-cycle detection via Graph.ancestors_of

## Summary
`validate` walks the parent spine with its own inline, cycle-guarded loop
(`trck:616-624`) to report parent cycles:

```python
for r in rows:  # parent cycles
    seen, cur = set(), r
    while cur and cur.parent is not None:
        if cur.parent in seen:
            errors.append(f"#{r.id:03d} is in a parent cycle")
            break
        seen.add(cur.parent)
        cur = by_id.get(cur.parent)
```

`Graph.ancestors_of` (`trck:427-439`) already walks the same spine with the same
cycle-breaking `seen` guard. The traversal logic is duplicated; only the "did we hit a
cycle?" signal differs.

## Acceptance criteria
- [ ] Parent-cycle detection in `validate` reuses `Graph`'s ancestor traversal rather
      than re-implementing the walk (e.g. `ancestors_of` returns or exposes enough to
      tell a clean spine from a cycle, or a small `Graph` predicate is added).
- [ ] The emitted error message and the set of issues flagged are unchanged.
- [ ] Existing parent-cycle tests still pass; add one if the behavior boundary moves.

## Notes
- One option: have `ancestors_of` (or a sibling helper) signal when the walk stopped
  because of a cycle vs. reaching a root, so `validate` can ask the graph instead of
  re-walking. Keep `ancestors_of`'s current return shape stable for its other callers
  (`match_closure`, `cmd_list`).
- Self-parent (`#n` parent `#n`) must still be reported.
