# deps: optimize node ordering to minimize edge span and crossings (local search)

## Summary
`trck deps` lays the DAG out top-to-bottom in a topological order, breaking ties by id.
Any topological order is *valid*, but the chosen order decides how long the edges are, how
many lanes are open at once, and how many cross. The shorter-edges work (#58 / `gwcr9qd`)
optimized **column assignment** — the horizontal half. This optimizes the **node order** —
the vertical/linearization half. Together they are the two halves of Sugiyama-style layered
graph drawing.

Idea: after the topological sort, run a **deterministic local search** — adjacent legal
transpositions (swap two neighbouring rows only when no dependency edge forces their order)
that lower a layout cost — to a local optimum, producing a visibly simpler graph than the
arbitrary id tie-break does.

## Acceptance criteria
- [ ] A layout **cost function**. Start with total edge vertical span — Σ over edges of
      `|row(dst) − row(src)|` (short edges ⇒ fewer/shorter open lanes). Evaluate whether to
      also weight open-lane width (cutwidth) or crossing count.
- [ ] **Deterministic** local search: a fixed canonical start order, adjacent *legal* swaps
      that strictly lower the cost, a deterministic tie-break for equal-cost moves, and **no
      RNG** — identical output on every run (stable screenshots, no churny diffs). A stable
      local optimum is fine; a different one each run is not.
- [ ] **Validity preserved**: every order produced is a valid topological order (a node is
      never placed before one of its prerequisites). Only nodes with no dependency path
      between them may be transposed.
- [ ] Applies to `deps` for the whole-graph view and the id-scoped / cone views; `list`,
      `tree`, and `ready` are unaffected.
- [ ] Tests: (1) output is always a valid topo order; (2) determinism — same input graph
      yields byte-identical output; (3) a fixture with known ordering slack shows the cost is
      ≤ the id-ordered baseline (a measurable improvement, not a regression).

## Notes
- This is the *vertex-ordering* half of the Sugiyama framework; #58 (`gwcr9qd`) did column
  assignment, #46 (`tazdgkg`) is the original renderer.
- Graphs are small (tens of nodes per connected component), so an O(n²)–O(n³) hill-climb is
  effectively free. A barycenter/median pre-pass can seed a good start order before swaps.
- **Orthogonal to id ordering.** The natural/numeric id sort used by `list`/`tree`/`index`
  (and as the canonical *start* order here) still stands; this only changes how the deps
  graph linearizes within the freedom the topo order leaves.
- Origin: surfaced when the int→string id change (#65) made the deps tie-break lexicographic
  rather than numeric. Rather than only restoring numeric order, optimize the order for
  readability — the tie-break stops mattering once the layout is cost-driven.
