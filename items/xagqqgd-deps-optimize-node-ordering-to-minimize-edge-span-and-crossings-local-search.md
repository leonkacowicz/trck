# deps: optimize node ordering to minimize edge span and crossings (local search)

## Summary
`trck deps` lays the DAG out top-to-bottom in a topological order, breaking ties by id.
Any topological order is *valid*, but the chosen order decides how long the edges are, how
many lanes are open at once, and how many cross. The shorter-edges work (#gwcr9qd / `gwcr9qd`)
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
- This is the *vertex-ordering* half of the Sugiyama framework; #gwcr9qd (`gwcr9qd`) did column
  assignment, #tazdgkg (`tazdgkg`) is the original renderer.
- Graphs are small (tens of nodes per connected component), so an O(n²)–O(n³) hill-climb is
  effectively free. A barycenter/median pre-pass can seed a good start order before swaps.
- **Orthogonal to id ordering.** The natural/numeric id sort used by `list`/`tree`/`index`
  (and as the canonical *start* order here) still stands; this only changes how the deps
  graph linearizes within the freedom the topo order leaves.
- Origin: surfaced when the int→string id change (#dscmxng) made the deps tie-break lexicographic
  rather than numeric. Rather than only restoring numeric order, optimize the order for
  readability — the tie-break stops mattering once the layout is cost-driven.

## Outcome

Done in `_shorten_lanes` (`src/trck/render.py`), fed the order `_graph_topo` produces. Total
lane length on this repo's largest component went 89 → 74, and 17 → 16 on another; the
remaining three were already optimal.

Two departures from the plan above, both forced by measurement:

- **Relocations, not adjacent transpositions.** The proposed neighbourhood turned out to be a
  no-op: the DFS-locality order is *already* a local minimum under adjacent legal swaps, on
  every component here — zero improving moves. Worse, from random start orders adjacent swaps
  converge to a mean cost of 108 against the existing heuristic's 89. Moving a node to any
  legal slot is what actually finds anything, and it reaches 74 from all 200 random starts
  tried as well as from the real start order, which suggests 74 is the global optimum.
- **No barycentre pre-pass.** Unnecessary once the neighbourhood is right — the existing
  order is a good enough seed that the search converges in four passes and twelve moves.

The "evaluate whether to also weight cutwidth or crossings" question resolved as *no*.
Measured across the whole view, the reorder left max gutter width (21) and `┼` count (10)
untouched — both are structural here, driven by a ten-child fan-in that no ordering avoids.
Span alone produced the visible win: the demand-cone chain now leads its component at depth 0
with its lanes closing immediately, where before it sat at column 3 with three idle lanes
running beside it for six rows.

What made it viable is that the cost, gathered per node rather than per edge, is
`sum(pos[v] * (indeg[v] - outdeg[v]))` — linear in the positions. A relocation shifts one
contiguous block by a single row, so a candidate's delta is a prefix-sum lookup instead of a
walk over the edges. The note above that "an O(n²)–O(n³) hill-climb is effectively free" was
too optimistic without that: recounting per candidate costs 5s at 80 nodes and 29s at 120,
against 2.5ms and 9ms with the delta. So no size cap was needed — 400 nodes settle in 280ms.
