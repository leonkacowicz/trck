# trck-html: route layer-skipping graph edges through dummy nodes

## Summary
The browser graph assigns each node a layer by longest path and draws every edge as one
bezier. An edge whose endpoints are more than one layer apart therefore flies over the rows
between them without holding a slot in any of them, and nothing in the layout can see it:

- `crossings()` skips such edges entirely when scoring a row order, because there is no
  index to compare them at. So they cross freely and the ordering never learns of it.
- `orderRows` and `refine` optimise against that same blind score, so a long edge cannot
  influence — or be improved by — either.
- The x-relaxation gives long edges only their endpoints, so they bow across the layout
  instead of running down a channel of their own.

Standard Sugiyama solves all three the same way: split an edge spanning layers L..M into
unit-length segments through a placeholder node in each intervening row. The placeholders
order and place like any other node, they just draw as a line rather than a box.

This is what the remaining crossings need. On this repo's graph the sweeps and the local
search both bottom out at 6 crossings, and brute force over all 240 orderings of the one
component that still crosses confirms 6 is the floor for the current model. Those 6 are
structural: without routing slots there is no ordering that removes them.

## Acceptance criteria
- [ ] An edge spanning more than one layer is represented internally as a chain of
      unit-length segments through one placeholder per intervening row.
- [ ] Placeholders take part in row ordering and x-placement exactly as real nodes do, so
      `crossings()` scores long edges and `orderRows`/`refine` can act on them. The
      layer-skipping exemption in `crossings()` and its comment come out.
- [ ] Placeholders render as the edge's own path, not as boxes: no bullet, no label, no hit
      target, and clicking through them does nothing.
- [ ] A long edge is drawn through its placeholders' x positions rather than as a single
      bezier between endpoints, so it runs in a channel instead of bowing across the rows.
- [ ] The known-floor case improves: this repo's graph drops below 6 crossings, or the new
      floor is established by brute force and recorded here.
- [ ] Cost stays inside the budget the refinement already respects. Placeholders inflate the
      node count — a 3-layer span adds 2 — so re-measure and revisit `REFINE_MAX`, which is
      set at 40 real nodes today.
- [ ] Tests drive the layout under node, as the existing ones in `TestGraphLayout` do, with a
      fixture whose long edge provably cannot be untangled without placeholders.

## Notes
- Touches `orderRows`, `crossings`, `refine`, `layoutComponent` and the edge-drawing loop in
  `renderGraph`, all in `tools/trck-html`.
- Straightening long edges is a genuine second win, not a side effect to ignore: a chain of
  placeholders gives the barycentre relaxation intermediate points to align, which is how
  layered drawings get their long edges to run straight.
- Applies to the browser graph only. The CLI `deps` gutter is one node per row with lanes, so
  it has no layers to skip and no analogue of this (see `xagqqgd`).
- Ought to land before `budhpcw` (hover highlighting). Not a hard dependency — hover can be
  built on today's model — but a long edge becomes several path elements here, and highlight
  logic written against one-path-per-edge would need reworking to group them.
