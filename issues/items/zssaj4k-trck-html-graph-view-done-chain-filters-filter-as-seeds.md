# trck-html: graph view — done-chain filters + filter-as-seeds

## Summary

Two rough edges in the v3 dependency-graph view:

1. It ignores the filter bar. The search/status/priority filters should **seed** the graph
   — show the union of the matching issues' dependency lines (prerequisites + dependents
   cones), like `trck deps <id>` generalised to many seeds.
2. No done-filtering. Mirror the CLI's `deps` flags with two graph-pane controls:
   **include done chains** and **omit done**.

## Design (JS; browser-verified + node --check)

Mirrors the engine's `filter_deps_graph_ids(omit_done, include_done_chains)`:

- **Toolbar** in the graph pane: two checkboxes.
  - `include done chains` — default **checked** (show everything). When unchecked, drop
    connected components whose nodes are all terminal (the engine's default hide).
  - `omit done` — default unchecked. When checked, drop all terminal nodes.
- **Filter-as-seeds:** when the filter bar is active, `seeds` = graph nodes (edge
  endpoints) that `matches()`; the visible set becomes the union of each seed's cone
  (reachable up via blockers + down via dependents). When inactive, the whole graph shows.
- Done-filtering is applied to the seeded/whole set; components, layout, edges, and nodes
  are all recomputed over the surviving subset (no synthetic edges across dropped nodes).
- The graph re-renders on filter change (already wired via `renderActiveView`) and on
  toggling either checkbox.

## Acceptance criteria

- [ ] Graph pane has `include done chains` (default on) and `omit done` (default off) controls.
- [ ] Unchecking `include done chains` hides fully-terminal components; `omit done` hides all done nodes.
- [ ] With the filter bar active, the graph shows the union of the matching issues' dependency cones.
- [ ] Empty states handled (no matching seeds / everything filtered out).
- [ ] v1–v6 behaviour unchanged; full suite (+ node --check) + `build.py --check` green.

## Notes

Graph filtering/layout is client-side JS, so Python tests cover the control presence and the
`node --check` syntax guard; cone/done-filter correctness is browser-verified. Slight divergence
from the CLI: done-chain hiding is offered even in seeded mode (the CLI disables it when scoped)
— kept uniform for predictability. Parent: epic #fkrp9dh.
