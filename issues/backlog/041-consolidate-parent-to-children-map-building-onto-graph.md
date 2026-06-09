# Consolidate parent-to-children map building onto Graph

## Summary
The `parent_id → [children]` map is built from scratch in at least three places, even
though `Graph` already computes and exposes exactly this:

- `Graph.__init__` (`trck:403-405`) — the canonical `_children` map, surfaced via
  `children_of()`.
- `validate` (`trck:629-632`) — rebuilds its own `children` dict, *despite already
  holding a `Graph g`* constructed at `trck:568`.
- `generate_summary` (`trck:785-788`) — builds another `children` dict by hand, and
  `leaf_rollup` (`trck:764`) then threads that dict plus a `term` lambda through its
  recursion instead of using `Graph`.

This is pure duplication of derived state: three hand-rolled copies of a map the read
view already owns. Consolidating them onto `Graph` removes the copies and keeps the
parent/child relationship defined in exactly one place.

## Acceptance criteria
- [ ] `validate` uses the `Graph g` it already builds instead of its own `children`
      dict.
- [ ] `generate_summary` obtains children via `Graph` (e.g. `load_graph`/`children_of`)
      rather than building a local map.
- [ ] `leaf_rollup` derives children from `Graph` instead of taking a `children` dict
      argument (or is otherwise reworked so no caller hand-builds the map).
- [ ] No remaining hand-rolled `parent → children` construction outside `Graph`.
- [ ] Full test suite passes; SUMMARY output and `check` results are unchanged.

## Notes
- `Graph.children_of` returns id-sorted lists; confirm SUMMARY ordering is preserved
  (it already sorts kids by id at `trck:814`).
- Watch the `term`/`is_terminal` plumbing — `Graph.is_terminal(row)` can replace the
  `term = lambda …` closures once children come from the graph.
