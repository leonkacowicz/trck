# Graph: add ancestor-closure, match-set and sibling-sort traversal

## Summary

Phase 3 of #qapvxpz. Add the traversal the merged browse verb needs onto the `Graph` class,
unit-tested in isolation with no command wiring. Demand-driven entirely by #qapvxpz (no other
consumer), so it is a #qapvxpz child even though the code lands in the `Graph` class — see the
spec's placement note: `docs/specs/2026-06-09-graph-derived-view-design.md`.

Surface to add:
- `ancestors_of(r)` — the parent spine up to a root (dangling parent ⇒ stop, treat as root).
- a match-closure helper — "node matches the filter **or** has a descendant that matches",
  yielding the shown set plus the dimmed-ancestor set.
- sibling-sort — order a sibling group by the chosen key, applied recursively.

## Acceptance criteria
- [ ] `ancestors_of(r)` returns the parent spine; safe on dangling-parent and cyclic data.
- [ ] Match-closure helper returns the match set + the ancestor-context (to-be-dimmed) set.
- [ ] Sibling sort orders a sibling list by a supplied key without escaping the parent.
- [ ] Unit tests cover each against fixtures (deep nesting, dangling parent, cycle, a match
      deep under non-matching ancestors).
- [ ] No command wired yet; all tests green; `trck check` passes.

## Notes

Consumed by #ub2ssg4. Builds on the Graph substrate from #chzay3q/#n2gdhdd.
