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
- [x] `trck dep NNN --add MMM` rejects an edge that would create a cycle, with a
      clear error naming the edge, and does not write the index.
- [x] Self-edge (`dep N --add N`) is rejected.
- [x] `validate` reports a dependency cycle as an **error** (one per cycle, not one
      per node — mirror the parent-cycle treatment and align with #008).
- [x] Tests cover: 2-node cycle, longer cycle, self-edge, and a valid DAG (diamond)
      that is left untouched.
- [n/a] `trck new --depends`: a brand-new issue gets a fresh max+1 id, and no
      existing issue can depend on a not-yet-created id, so a `new` can never close a
      cycle (and `--depends` already rejects ids not in the index, so no self-edge
      either). No guard needed at create-time; the invariant is enforced at every
      `dep` add and by `validate`.

## Notes
- Implemented as two module-level helpers next to `parent_ids`:
  - `dep_would_cycle(by_id, src, dep)` — write-time guard: true if `src == dep` or
    `src` is already reachable from `dep`. Used by `cmd_dep`.
  - `find_dep_cycles(by_id)` — white/gray/black DFS returning each cycle once
    (deduped by node set). Used by `validate` to emit `dependency cycle: #a -> #b ->
    #a`.
- Pairs naturally with the existing dep-validation issues #008 / #009.
