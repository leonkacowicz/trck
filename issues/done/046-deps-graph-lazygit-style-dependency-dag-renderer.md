# deps --graph: lazygit-style dependency DAG renderer

## Summary
Add a `--graph` flag to `deps` that draws the `depends_on` DAG as a
one-row-per-node gutter, the way lazygit renders git history: every
merge/fork is co-located on its node's own row with box-drawing corners and
horizontal runs (no blank edge rows). This shows shared prerequisites
(diamonds) without the node duplication the parent-tree view would force.

## Acceptance criteria
- [x] `deps --graph` (no id) draws the whole DAG — every issue touching a
      depends_on edge — split into weakly-connected components, separated by a
      blank line so independent clusters don't read as one false chain.
- [x] `deps <id> --graph` scopes to just that issue's connected component.
- [x] An edge-less issue reports `(no dependencies)`; bare `deps` (no id, no
      `--graph`) errors helpfully instead of crashing. Existing per-issue
      `deps` behaviour is unchanged.
- [x] Nodes render prerequisites-first (topological); each lane keeps one
      colour for its whole descent so it can be traced through `┼` crossings.
- [x] Colour reuses the TTY-gated `paint()` seam (NO_COLOR / FORCE_COLOR);
      piped output stays plain. Renderer is pure and stdlib-only.
- [x] Covered by tests; full suite and `trck check` green.

## Notes
Implemented in commit 1991f1f. Engine: `graph_components`, `_graph_topo`,
`_graph_component_rows`, `render_graph`, `paint_lane` (+ extended `_ANSI`
palette / `_LANE_PALETTE`), and the `cmd_deps` graph branch with an optional
`id` (`nargs="?"`). Reuses the `Graph` view + a Kahn topo sort; does not touch
`print_rows` (gutter rows aren't issue rows). 14 tests in
`tests/test_graph_render.py`.

Design was prototyped first: a naive one-row-per-node attempt with diagonals
failed (a chain rendered as disconnected bullets), which led to the lazygit
corner-based cell model (glyph = f(connection set)). Validated against a real
127-issue / 76-edge external tracker before landing.
