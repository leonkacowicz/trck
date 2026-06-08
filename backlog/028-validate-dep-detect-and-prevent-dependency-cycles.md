# validate/dep: detect and prevent dependency cycles

## Summary
trck does not detect or prevent cycles in the `depends_on` graph. `cmd_dep`
(`trck`, ~L929) only checks that the target id exists before adding the edge, so
`dep 1 --add 2` followed by `dep 2 --add 1` is accepted. `validate` (~L398) walks
for **parent** cycles (~L450) and checks that every dep target exists (~L446) but
has **no dependency-cycle check** — `trck check` passes on a cyclic dep graph.

The asymmetry (parent cycles guarded, dep cycles not) is almost certainly
unintentional. A cyclic dep graph is an invariant violation and is a latent
infinite-loop hazard for any transitive `depends_on` traversal that lacks its own
`seen`-set guard. (Today `is_blocked`/`ready`/`next` only look one hop deep, so
they're safe, but the invariant isn't enforced.)

Defense in depth: catch it at **both** layers.

## Acceptance criteria
- [ ] `trck dep NNN --add MMM` rejects an edge that would create a cycle, with a
      clear error naming the cycle, and does not write the index.
- [ ] `trck new` with `--depends` likewise rejects an edge that would close a cycle.
- [ ] `validate` reports a dependency cycle as an **error** (one per cycle, not one
      per node — mirror the parent-cycle treatment and align with #008).
- [ ] Self-edge (`dep N --add N`) is rejected.
- [ ] Tests cover: 2-node cycle, longer cycle, self-edge, and a valid DAG that is
      left untouched.

## Notes
- The parent-cycle walk at `trck` ~L450-457 is a ready-made template; the same DFS
  with a recursion stack over `depends_on` closes the gap.
- Pairs naturally with the existing dep-validation issues #008 / #009.
