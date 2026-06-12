# deps: shorter edges / fewer crossings in the graph layout

## Summary
Make the `deps` graph "simpler on the eyes" by shortening edges and reducing
crossings, without growing the rendering code much (stdlib-only, single file).

Frame the layout as two independent layers:

1. **Node ordering** — the topological sort in `_graph_topo` (sets each edge's span).
2. **Lane assignment** — routing each edge through a side column in
   `_graph_component_rows` (given the spans from layer 1).

Findings from reading `trck`:

- **Layer 2 width is already optimal.** Lanes reuse the leftmost free (`None`) column
  (`bottom.index(None)`), which is greedy interval-graph colouring → minimum number of
  lanes for whatever order it's handed. No win available in lane *count*.
- **Layer 1 ordering is NOT span-aware.** `_graph_topo` is plain Kahn's algorithm with
  tie-break by id (`ready.sort()` then `ready.pop(0)`). Among equally-ready nodes, id
  order is visually arbitrary — this is the main source of long edges / extra
  simultaneously-open lanes / crossings.
- **Width-optimal ≠ crossing-optimal.** Leftmost-free reuse minimises lane count but can
  drag a long horizontal `bridge` across other lanes when a freed far-left column is
  grabbed for a target that sits to the right — an avoidable crossing at the *same* width.

The exact objective (minimise total edge length over valid topological orders) is the
topological-order-constrained Minimum Linear Arrangement — NP-hard — and true crossing
minimisation (the Sugiyama ordering step) is NP-hard too. So this issue is about cheap,
high-payoff heuristics, not optimal solvers.

Two improvements, independent and separately shippable:

- **(A) Lane-slot choice — same-width crossing reduction (pure win, low risk).** When
  opening a forked dependent lane, pick the free column *nearest `pos`* instead of the
  leftmost gap. Shortens bridges, cuts crossings, never changes lane count or node order.
- **(B) Span-aware order tie-break — bigger visual impact, but changes the listing.**
  Among ready nodes, prefer the one whose most-recently-placed predecessor sits lowest
  (shortest incoming edge) so lanes close fast → narrower + fewer crossings. Keep id as
  the final tie-break for determinism. **Caveat:** today's id tie-break floats older
  prerequisites (lower ids) to the top, giving a stable, predictable order; a span-aware
  order reshuffles the visible sequence. Validate before/after on the real `issues/`
  graph and confirm the trade is wanted.

## Acceptance criteria
- [x] (A) Forked lanes choose the free column nearest `pos`; lane *count* is unchanged
      for every existing graph (width is still greedy-optimal), and visible crossings are
      reduced or equal on the repo's own `deps` output. *(shipped — see Notes)*
- [ ] (B) `_graph_topo` tie-break prefers the ready node with the most-recently-placed
      predecessor, falling back to id for full determinism; output stays a valid
      topological order (every blocker above what it blocks).
- [ ] (B) is gated on a before/after comparison on the real `issues/` graph being judged
      an improvement — if it isn't, ship (A) alone and drop (B).
- [ ] Tests cover: lane count unchanged after (A) on representative DAGs; (B) still
      produces a valid topo order and is deterministic; a fan-in/fan-out case where (A)
      demonstrably shortens a bridge.
- [ ] No new third-party imports; remains within the single `trck` file.

## Notes
Relevant code (all in `trck`):

- `_graph_topo` (~line 887) — Kahn's sort, tie-break by id. Lever for (B).
- `_graph_component_rows` (~line 913) — per-node lane bookkeeping; `bottom.index(None)`
  (~935) is the leftmost-free slot choice to revisit for (A); `bridge()` (~949) is the
  horizontal run whose length (A) reduces.
- `graph_components` (~862), `render_graph` (~988), `_print_deps_graph` (~1610) — callers,
  unaffected by either change.

Discussion origin: layout was analysed as constrained MinLA (edge length) + interval
colouring (lanes). Conclusion: lane colouring is already optimal for width, so the gains
live in the ordering (B) and in same-width bridge shortening (A).

**Progress:** (A) shipped — the `started` k>0 branch now picks the free column nearest
`pos` (`min(free, key=…|c-pos|, c)`), covered by `test_reopened_lane_hugs_the_node_…`
and `test_nearest_gap_reuse_does_not_widen_the_graph`. (B) remains open; keeping the
issue in `backlog` until (B) is decided.
