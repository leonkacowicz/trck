# rust: the deps gutter renderer

## Summary
The lazygit-style DAG gutter. Contract, not implementation — it is printed to a terminal.

Port the whole pipeline, not just the drawing: topological order, the lane-shortening pass, lane
assignment, and colouring. The shortening pass is worth carrying across as designed — its cost
is linear in the positions, which is what makes a relocation's delta a prefix-sum lookup rather
than a walk over the edges, and without that it is unusably slow at any real size.

## Acceptance criteria
- [x] Edge set built and transitively reduced over exactly the ids being drawn.
- [x] Component grouping and separators.
- [x] Topological order with the DFS-locality tie-break, then lane shortening.
- [x] Lane assignment, bridges, and the per-lane edge-kind colouring that lets an inferred
      containment edge draw differently from an authored one.
- [x] The focal caret and the id-scoped cones.
- [x] Passes the `uq2zc2p` golden gutters byte for byte.

## Landed
`01157a9`. Conformance 11/12 -> **12/12**: the Rust engine passes every fixture. That says
as much about the fixtures as the engine — the verbs still unported have none.

**Verified far past the fixtures: 1014 invocations diffed against Python**, every issue in
both trackers crossed with every flag, byte-identical including exit codes. This is the
piece where that mattered most: lane assignment is a layout algorithm, and "looks right"
is not a standard it can be held to.

Two bugs the sweep caught, both invisible in a small case:

**`shorten_lanes` restarted its scan after every accepted move**, where Python continues
from `i+1` over the already-updated order. Same cost function, same termination argument —
but a different fixed point, so the two engines settled on different-but-equally-valid
layouts. Only a graph with enough slack to move more than once reveals it; the 23-node
component does, a three-node unit test does not. Worth remembering: for a local search,
porting the *cost* correctly is not enough — the traversal is part of the answer.

**`--full` needs its component computed over every issue**, not over the overview set. The
overview drops components the bare view suppresses, so it could lose the focal node itself.

`dependency_line` came along, since `deps ID` is defined in terms of it: both sweeps cross
containment but neither turns around, so siblings stay cousins.
