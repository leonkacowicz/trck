# deps: order graph components by demand (most-demanded cluster first)

## Summary
`deps` splits the graph into weakly-connected components and currently orders them by their
smallest member id (`gutter::components`, `comps.sort_by(|a, b| a.first().cmp(&b.first()))`).
Order them instead by the highest demand vector among their members (id as the final tie-break),
so the cluster containing the most important work renders on top — the same importance signal
`ready`/`next` already give.

This is a **Pareto improvement**: components are independent islands with no topological edge
between them, and `shorten_lanes` never reorders across component boundaries, so a
demand-based component order costs nothing in gutter/lane length — it only changes which
cluster leads. Verified on a toy DAG: reordering components left the total idle-lane length
unchanged.

Scope is deliberately just the *macro* order (which component first). The *within*-component
layout stays exactly as today (DFS-locality + `shorten_lanes`). Reordering inside a component
by demand is the separate, non-free follow-up [[deps-demand-innermost-tie-break-within-component]].

## Acceptance criteria
- [ ] Components are ordered by the max demand vector of their members, id-tie-broken.
- [ ] Demand reaches the ordering as an `{id: demand_vector}` map computed once, rather than the
      component code querying the `Graph` per comparison. `components` takes only `ids` + edges
      today; its callers in `render_graph` and `cmd_deps` already hold a `&Graph`, so the map is
      built there and passed down. It is the shared plumbing the tie-break follow-up reuses.
- [ ] Within-component row order and total lane length are unchanged from current output.
- [ ] Deterministic (id final tie-break).
- [ ] Conformance fixture covering a multi-component graph where demand order differs from
      min-id order.
- [ ] `ratchet generate` run and the regenerated `quality-report.json` staged with the change.

## Notes
- Current code: `gutter::components` at `src/gutter/mod.rs:159`, ordered at `:188`. Callers:
  `render_graph` (`:408`), `overview_ids` (`:429`), and `src/query/mod.rs:208`.
- Demand vector definition: `Graph::demand_vector` (`src/graph.rs:241`) and the reusable
  `demand_vector_with` (`:245`), which takes a prebuilt reverse-edge map — per-priority cone
  population, compared lexicographically. `ranked_ready` (`:277`) is the existing example of
  computing `demand_edges()` once and reusing it across comparisons.
- Only `components`' final ordering changes; `overview_ids` and `query/mod.rs:208` use components
  as a *set* (membership), so they are unaffected by the order.
- The former deferral is resolved: the Rust port epic `#sp2rwzx` is done and the Python engine is
  gone, so this lands in the Rust engine only, with nothing to mirror.
