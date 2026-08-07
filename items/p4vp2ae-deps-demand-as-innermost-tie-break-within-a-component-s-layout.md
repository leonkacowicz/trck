# deps: demand as innermost tie-break within a component's layout

## Summary
Inside a component, order rows by demand **only where it costs no lane length** — i.e. demand as
a tertiary key: topological validity first, crossing/lane-length minimization second, demand
third. Today the ties `shorten_lanes` is indifferent to (mostly symmetric sibling leaves) are
broken by ascending id — arbitrary. This replaces that arbitrary tiebreak with a meaningful one
(demand, then id).

Marginal, deliberately conservative polish: anything demand "wants" that would lengthen a lane is
rejected by construction (that is what "after crossings" means), so this only claims the *free*
part of demand ordering. Depends on [[deps-order-graph-components-by-demand-most-demanded-cluster-first]]
for the `{id: demand_vector}` plumbing.

## Approach — cheap path (recommended)
Do **not** touch `shorten_lanes`' cost function. Instead, in `topo`, change the innermost
tie-break of the DFS-locality seed (the `newly` set and the roots, currently sorted by id) from
`id` to `(-demand, id)`. `shorten_lanes` runs unchanged afterward: where a demand-driven
arrangement costs a lane it finds a reducing move and overrides it; where it is lane-neutral it
never moves it, so the seed's demand order survives. Net effect = "topo -> crossings -> demand".
~one line, riding on the component-ordering issue's demand map.

Rejected — expensive path: making `shorten_lanes`' objective lexicographic `(laneLength,
demandDisorder)`. It is tractable (the disorder term `sum(pos*-demand)` is linear, so the O(1)
prefix-sum delta and termination survive) and is safe-on-gutter by construction, but it is
surgery on delicate, perf-tuned, panic-free code and re-opens its termination / delta proofs.
Not worth it for an exact tertiary key when the primary objective is already only a
first-improvement heuristic.

## Acceptance criteria
- [ ] `topo` (`src/gutter/mod.rs:277`) breaks the freed-set / roots tie by `(-demand, id)` instead
      of `id`; `shorten_lanes` untouched. Both seeds: the initial ready stack (`:304`) and the
      `newly` set (`:316`). Both sort then `reverse()` into a LIFO so the pop order is ascending —
      the new key has to keep that inversion.
- [ ] **Gutter must not regress.** DFS-locality was chosen as a good *seed* for `shorten_lanes`;
      perturbing the sibling order could steer first-improvement into a worse local minimum.
      Verify total lane length does not increase on the repo's own `deps` graph and across the
      conformance fixtures. If a regression shows up, reconsider the expensive path
      (safe-on-gutter by construction).
- [ ] Determinism preserved (id as the final-final tie-break).
- [ ] `ratchet generate` run and the regenerated `quality-report.json` staged with the change.

## Notes
- This is the "3rd tie-break, after topo and after lane crossings" idea. It is genuinely marginal;
  ship the component-ordering issue first and only pick this up if id-ordered siblings visibly
  bother us in practice.
- The former deferral is resolved: the Rust port epic `#sp2rwzx` is done and the Python engine is
  gone, so this lands in the Rust engine only, with nothing to mirror.
- Demand vector: `Graph::demand_vector` (`src/graph.rs:241`), or `demand_vector_with` (`:245`)
  against a prebuilt reverse-edge map, as `ranked_ready` (`:277`) does.
