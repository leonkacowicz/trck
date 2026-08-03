# conformance: convert the graph-semantics tests

## Summary
These are written against the internal `Graph` API but specify observable behaviour — readiness,
demand ranking, effective dependencies, parent rollup. `test_graph.py` alone is 73 tests that
barely touch stdout yet almost entirely describe what `ready`, `next` and the row annotations
must do.

Converting them means re-expressing the contract at the boundary where it is visible: not
"`demand_vector` returns `[0,4,9,1,0,0]`" but "given this graph, `ready` returns these issues in
this order, with these `↑` markers".

## Acceptance criteria
- [ ] Readiness: unblocked non-terminal leaves only, honouring inherited dependencies and
      non-actionable statuses.
- [ ] Demand ranking, including the case that motivates it — a medium task blocking an urgent
      one outranking a high task blocking nothing.
- [ ] The `↑<priority>(#id)` annotation, including when it is *not* emitted.
- [ ] Effective dependencies through the hierarchy, both directions.
- [ ] Parent rollup percentages and the leaf/parent distinction.
- [ ] Cycle rejection, direct and through the hierarchy.
