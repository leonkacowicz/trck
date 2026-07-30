# v3: dependency graph view (clickable, jump between blockers/blocked)

## Summary

Add a second view to the SPA: a **dependency graph** rendered as an SVG layered DAG,
alongside the existing list. Toggle `[ list | graph ]`. Nodes are issues; edges are
authored dependencies (blocker above what it blocks); clicking a node selects it and
updates the detail panel, so you can jump between blockers and blocked work.

## Design

**Data (testable Python seam):** `build_model` gains `model["edges"]` — a precomputed
list of authored dependency edges `{"from": blocker, "to": blocked}` derived from each
issue's `requires`. Direction matches the engine's `deps` (a blocker sits above what it
blocks). Containment (parent/child) edges are **not** included in v3 — authored deps only.

**View (JS; browser-verified):**
- A `[ list | graph ]` toggle in the filter bar switches the left pane between the list
  and an `<svg>` graph canvas; the detail panel stays.
- Graph nodes = the issues that appear in at least one edge (isolated issues are omitted,
  mirroring the engine's whole-graph `deps`).
- Layout: longest-path layering (layer = longest chain of predecessors), laid out
  top→bottom; nodes ordered by id within a layer. Straight edges with arrowheads,
  blocker→blocked. Cycle-guarded (authored deps are acyclic per `trck check`, but the
  layering never loops even on malformed data).
- Nodes are clickable → `select(id)`: detail panel updates, selected node highlighted;
  terminal (done) nodes are dimmed. Empty graph shows a "no dependencies" message.
- The list's search/status/priority filters affect the list only; the graph shows the
  full authored-dependency graph. (Scoping/dimming the graph by filter is a later polish.)

## Acceptance criteria

- [ ] `model["edges"]` lists authored dependency edges as `{from, to}` (blocker→blocked).
- [ ] Containment (parent/child) produces no edge.
- [ ] Rendered document includes the graph container + the list/graph view toggle.
- [ ] Graph view draws clickable nodes that select the issue and update the detail panel.
- [ ] v1/v2 behaviour unchanged; full suite + `build.py --check` green.

## Notes

The SVG layout is client-side, so Python tests cover the `edges` data contract and the
presence of the view UI; the visual layout/interaction is verified by opening the file
(same manual-check limitation as v1/v2). Containment edges and filter-scoped graphs are
deliberately deferred. Parent: epic #fkrp9dh.
