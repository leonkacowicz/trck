# rust: graph semantics — parents, effective deps, demand, readiness

## Summary
The derived view over the loaded index: hierarchy, dependencies that climb it, cycle detection,
the demand cone and readiness. The most semantically dense part of the engine and the part most
worth getting under the conformance suite before anything renders it.

## Acceptance criteria
- [ ] Parent/child structure, ancestors, descendants, subtree, rollup percentages.
- [ ] Effective dependencies in both directions — depending on a parent depends on its whole
      subtree; a parent's dependencies are inherited by every child.
- [ ] Cycle rejection, direct and through the hierarchy.
- [ ] Demand vector and the ranking built on it.
- [ ] Readiness, honouring inherited blocking and non-actionable states.
- [ ] Passes the `88ax67p` fixtures.
