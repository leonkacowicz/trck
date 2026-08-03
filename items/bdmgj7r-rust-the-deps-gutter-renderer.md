# rust: the deps gutter renderer

## Summary
The lazygit-style DAG gutter. Contract, not implementation — it is printed to a terminal.

Port the whole pipeline, not just the drawing: topological order, the lane-shortening pass, lane
assignment, and colouring. The shortening pass is worth carrying across as designed — its cost
is linear in the positions, which is what makes a relocation's delta a prefix-sum lookup rather
than a walk over the edges, and without that it is unusably slow at any real size.

## Acceptance criteria
- [ ] Edge set built and transitively reduced over exactly the ids being drawn.
- [ ] Component grouping and separators.
- [ ] Topological order with the DFS-locality tie-break, then lane shortening.
- [ ] Lane assignment, bridges, and the per-lane edge-kind colouring that lets an inferred
      containment edge draw differently from an authored one.
- [ ] The focal caret and the id-scoped cones.
- [ ] Passes the `uq2zc2p` golden gutters byte for byte.
