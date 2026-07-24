# deps: transitive reduction of the rendered graph (display-only, after done-filtering)

## Summary

Drop every edge that is already implied by a longer path: if `A` depends on both `B` and
`C`, and `B` depends on `C`, draw `A <- B <- C` and omit `A <- C`. On a DAG the transitive
reduction is **unique**, so there is no arbitrary choice to make, and at trck's scale the
naive O(V·E) computation is free.

This is not only cosmetic. Once #zhhxgcw and #zbkkc2a add inferred edges, redundancy is
systematic rather than occasional — any authored edge already implied by an ancestor's edge
becomes visible clutter. Reduction is what keeps the denser graph legible.

It also shrinks lane count and horizontal bridge length directly, which is a bigger
structural win than reordering nodes — so #xagqqgd (local-search node ordering) should be
re-scoped after this lands, to optimise only what survives.

## Acceptance criteria
- [ ] The rendered graph omits any edge `u -> v` for which another path `u ~> v` of length
      >= 2 exists **within the drawn id set**.
- [ ] Reduction runs **after** `filter_deps_graph_ids`, never before — see the trap below.
- [ ] Display-only: `index.jsonl` is not rewritten, and `trck dep --remove` is still the
      only way to delete an edge.
- [ ] Tests cover the diamond case, the done-filtering interaction, and idempotence.

## Notes

- **Ordering trap (the one real correctness risk).** If `B` is done and hidden while
  `A -> C` was reduced away as redundant via `A -> B -> C`, then `A` and `C` render as
  unconnected and the graph *lies*. Reducing the already-filtered id set cannot produce
  that: a path only justifies dropping an edge if the path is itself drawn. The same
  ordering applies to `--depth` if #9ax2ny2 ever lands, and to any future filter that
  removes nodes.
- `filter_deps_graph_ids` (`trck:1342`) shrinks the id set and lets `render_graph`
  recompute components, so the natural insertion point is between that call and
  `render_graph` in `_print_deps_graph` (`trck:2133`).
- Reduce whichever graph is being drawn — authored, or authored + inferred. Reducing one
  and rendering the other reintroduces the trap above in a subtler form.
- Hiding redundant edges without ever surfacing them lets the index accumulate cruft that
  only bites when someone later removes the covering edge and a hidden constraint silently
  reappears. #wsqfwc6 is the counterweight.
