# deps: order graph components by demand (most-demanded cluster first)

## Summary
`deps` splits the graph into weakly-connected components and currently orders them by their
smallest member id (`graph_components`, `key=min`). Order them instead by the highest demand
vector among their members (id as the final tie-break), so the cluster containing the most
important work renders on top — the same importance signal `ready`/`next` already give.

This is a **Pareto improvement**: components are independent islands with no topological edge
between them, and `shorten_lanes` never reorders across component boundaries, so a
demand-based component order costs nothing in gutter/lane length — it only changes which
cluster leads. Verified on a toy DAG: reordering components left the total idle-lane length
unchanged.

Scope is deliberately just the *macro* order (which component first). The *within*-component
layout stays exactly as today (DFS-locality + `shorten_lanes`). Reordering inside a component
by demand is the separate, non-free follow-up [[deps-demand-innermost-tie-break-within-component]].

## Acceptance criteria
- [ ] `graph_components` orders components by max demand vector of their members, id-tie-broken,
      in both the Python (`src/trck/render.py`) and Rust (`crates/trck/src/gutter.rs`) engines.
- [ ] Demand is threaded to the render layer as a `{id: demand_vector}` map computed once at the
      `cmd_deps` call site (the renderer must not reach into the `Graph`; it only sees `idset` +
      edge maps today). This map is the shared plumbing the tie-break follow-up reuses.
- [ ] Within-component row order and total lane length are unchanged from current output.
- [ ] Deterministic (id final tie-break); conformance suite + `--compare-bin` agree between engines.
- [ ] Test covering a multi-component graph where demand order differs from min-id order.

## Notes
- Current code: `graph_components` at `src/trck/render.py:264` (`return sorted(comps, key=min)`,
  line 290); Rust equivalent in `crates/trck/src/gutter.rs`.
- Demand vector definition: `Graph.demand_vector` (`src/trck/graph.py:243`); per-priority cone
  population, compared lexicographically. Rust `crates/trck/src/graph.rs:266`.
- **Deferred pending the decision on decommissioning the Python engine** (Rust port epic
  `#sp2rwzx`). If Python is retired, this lands in the Rust engine only; if both live, it must be
  mirrored byte-for-byte. Do not start until that call is made.
