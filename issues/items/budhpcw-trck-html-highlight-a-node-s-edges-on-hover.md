# trck-html: highlight a node's edges on hover

## Summary
In the browser graph you can see that a node has edges, but not which ones. Following a line
out of a busy node means tracing it by eye through everything else leaving the same point.
Hovering a node should light up the edges incident to it.

Hovering the **node** rather than the edge is the point. It is a large hit target, it needs no
extra elements, and it answers the question people actually have — *what blocks this, and what
does it block* — rather than *where does this particular line go*. Hovering the edge itself
would mean a transparent fat companion path per edge to make a 1.5px stroke hittable, doubling
the path count for a narrower question.

## Acceptance criteria
- [ ] Hovering a node highlights the edges incident to it; leaving restores them. Built from
      an edge→node index at render time, with no extra SVG elements per edge.
- [ ] The arrowheads follow. `#arrow` is a shared `<marker>` with a hardcoded
      `fill: var(--muted)`, so a highlighted edge keeps a grey head unless this is handled —
      either `fill: context-stroke` (SVG 2, fine in current Chrome/Firefox/Safari) or a second
      `#arrow-hi` def swapped in by `.gedge.hi { marker-end: url(#arrow-hi) }`.
- [ ] Highlighted edges paint above unhighlighted ones. SVG has no z-index, so this means
      re-appending them, and edges are currently emitted before nodes as one batch.
- [ ] Decide and record whether blockers and dependents are distinguished (two colours, or
      direction shown some other way) or both simply take the accent.
- [ ] Hover styling does not fight the existing selected state, which already takes the accent
      on `.gnode.sel rect`.

## Notes
- All in `tools/trck-html`: the `.gedge`/`.gnode` rules and `renderGraph`'s edge loop.
- Deliberately hover, not selection. `select(id)`/`state.selected` already exist and extending
  them would be the smaller change, but selection persists and drives the detail pane; hover is
  the transient "what is this connected to" gesture and leaves what the user is reading alone.
- Ought to follow `6yptz6p` (dummy nodes). Not blocked by it — this works fine on today's model
  — but that change turns a long edge into several path elements, so highlight logic written
  against one-path-per-edge would need reworking to group the segments of one edge.
- Origin: the question that opened the session in which the graph's arrowheads, row gap,
  barycentre placement and crossing minimisation were all fixed. It was deferred each time in
  favour of the layout work underneath it.
