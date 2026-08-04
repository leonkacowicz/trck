# rust: graph semantics — parents, effective deps, demand, readiness

## Summary
The derived view over the loaded index: hierarchy, dependencies that climb it, cycle detection,
the demand cone and readiness. The most semantically dense part of the engine and the part most
worth getting under the conformance suite before anything renders it.

## Acceptance criteria
- [x] Parent/child structure, ancestors, descendants, subtree, rollup percentages.
- [x] Effective dependencies in both directions — depending on a parent depends on its whole
      subtree; a parent's dependencies are inherited by every child.
- [x] Cycle rejection, direct and through the hierarchy.
- [x] Demand vector and the ranking built on it.
- [x] Readiness, honouring inherited blocking and non-actionable states.
- [ ] Passes the `88ax67p` fixtures. **Not yet possible** — no verb renders any of this,
      so there is nothing for a fixture to observe. Verified differentially instead (below),
      which is stronger than the fixtures would have been at this stage.

## Landed
`graph.rs` (`b44e24b`), 25 tests, clippy clean.

**Verified differentially against the Python engine over this repo's real 195-issue
graph** — leaf, blocked, ready, rollup percentage, lifted dependencies, demand cone size,
demand source, and the complete `ready` ranking, identical for every issue. Hand-written
cases cover shapes someone thought of; a real graph with real epics and real inherited
edges is where a subtly wrong lifting rule shows up. The dump is an opt-in test
(`TRCK_DUMP_GRAPH=path cargo test dump_real_graph`) so it can be rerun.

**The tests caught the obvious wrong guess.** An issue depending on its own ancestor is
*not* refused by reachability: with no authored edges anywhere, `would_cycle` has nothing
to reach through and reports the edge as fine. Overlapping subtrees are a separate
invariant — `containment` — and Python checks it separately for exactly that reason. The
test now pins both halves rather than the conclusion, so the next reader does not
re-derive it wrongly.

Cycle detection is iterative rather than recursive. Recursion is shorter and blows the
stack on the deep hierarchy a malformed index can produce, which is the case this code
exists to report on.
