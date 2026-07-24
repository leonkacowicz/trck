# deps: draw inherited dependency edges (child -> ancestor's out-of-subtree deps)

## Summary

Second family of inferred edges: for every issue, draw an edge to each dependency target of
each of its ancestors. This is the source-side lifting rule the engine already implements in
`lifted_deps` — a parent's dependencies are inherited by every descendant — made visible.

Targets are always outside the child's own subtree, because an authored edge between an
ancestor and a descendant is already rejected as a cycle. So no inherited edge can point
inward, and none can close a loop.

Without this, a child blocked purely by an ancestor's dependency renders with no outgoing
edge and reads as actionable, while `ready`/`next` correctly withhold it — the graph would
understate the constraint. Containment edges alone do not fix this: `P -> C` and `P -> X`
say "P needs both", which is strictly weaker than the truth, "C needs X".

## Acceptance criteria
- [ ] Each issue gets an inferred edge to every dependency target of every ancestor.
- [ ] Inherited edges are visually distinguishable from authored ones.
- [ ] A child blocked only through an ancestor's dependency shows that blocker in the graph.
- [ ] No inherited edge points into the child's own subtree (assert it; it should be
      impossible given the existing cycle rules).
- [ ] `index.jsonl` unchanged.

## Notes

- Reuse `Graph.lifted_deps` (`trck:655`) rather than re-walking the spine — it is already
  the single shared lifting primitive, and `is_blocked` reads the same source.
- **This is what creates the fanout that #wr8ybmk exists to manage.** Concretely, with `P`
  an epic, children `C1..Cn`, and an authored `P -> X`, the drawn edge set becomes
  `P -> Ci` (containment), `Ci -> X` (inheritance), and `P -> X` (authored) — but `P -> X`
  is now implied by `P -> C1 -> X`, so #atk4umk deletes it and replaces one parent-altitude
  edge with `n` child edges. That fires for *every* parent-authored dependency, i.e.
  precisely the edges the docs tell you to prefer. Do not land this without #wr8ybmk close
  behind, or the common case gets denser instead of sparser.
- Rejected alternative: protect authored edges from reduction. It keeps `P -> X` *and* the
  fanout — worst of both.
- Rejected alternative: skip this issue and let the `needs #…` row annotation carry
  inherited blocking. That annotation is authored-only today (#9jgarpa), and even fixed it
  gives no ordering — which is the whole point of the graph.
