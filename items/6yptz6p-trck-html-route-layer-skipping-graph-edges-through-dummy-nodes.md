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

## Outcome

`splitLongEdges` builds the routed graph; `layoutComponent` lays out real nodes and
placeholders together and returns the placeholder positions as `bends`, which the edge loop
in `renderGraph` draws through. An edge is still one path — a curve into each row it
crosses, straight down that row's band, then a curve on — so a routed edge reads like a
short one and `orient: auto` still aims the head down.

Placement had to learn variable widths. A placeholder is a line, not a box, so it claims no
column of its own; spacing became centre-to-centre and per-pair rather than one uniform
step. The isotonic fit survived that unchanged — the substitution still lands on
"non-decreasing", just against per-row offsets instead of `i * STEP`.

**The crossing floor did not move here, and that is now proven rather than assumed.** With
placeholders this repo's tangled component is 2x7x1 (was 2x5x1), and brute force over all
10,080 orderings still gives 6. Those crossings are structural: two blockers feeding
overlapping subsets of the same children, which routing slots cannot help.

The example tracker is where the win landed, and only after a second change. Splitting
alone left it at 2 crossings — but brute force over its 2x7x5x2 component said the optimum
was now **0**, a gap that did not exist before because the long edge was not being counted
at all. Closing it needed the ordering sweeps to use a **median** barycentre rather than a
mean: an outlier drags a mean, so a row gets ordered by a position no neighbour occupies,
and the extra neighbours placeholders introduce make that bite. Median is also the variant
with a proven bound (Eades-Wormald). With both changes the example graph draws at 0.

Relocation could not close that gap on its own — nor could adding pairwise exchange to its
neighbourhood, which was tried and rejected: relocation already subsumes adjacent swaps, and
exchange found nothing further.

`REFINE_MAX` stays at 40 but now counts placeholders, so a component full of long edges
reaches it at fewer issues. Re-measured on a dense synthetic *with* long edges: 37 total
nodes costs 10.9ms, still inside a frame. Whole-graph layout went 0.21ms to 1.05ms here and
0.78ms to 1.09ms for the example — more nodes to order, still far under budget.

The layer-skipping guard in `crossings` stayed rather than being deleted as the criteria
said. Every edge reaching it is unit-length now, so it never fires; keeping it means an
unsplit edge is ignored rather than silently scored against an index from the wrong row.
Its comment says so.
