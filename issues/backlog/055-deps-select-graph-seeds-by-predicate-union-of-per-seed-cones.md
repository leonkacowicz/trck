# deps: select graph seeds by predicate (union of per-seed cones)

## Summary
Generalize `deps <id>` (single-issue scope, #047) to seed the graph from a *set* of
issues chosen by a predicate. Conceptually: run `deps NNN` for every issue NNN matching
the predicate, then merge the resulting graphs by deduping nodes and taking the union of
edges.

This is well-specified: each `deps NNN` yields a reachability cone (V_NNN, E_NNN), and
set union (V = ⋃V_NNN, E = ⋃E_NNN) is commutative/associative/idempotent — so the result
is order-independent and deterministic. The merged graph is exactly **the reachability
cone of the seed set** {NNN : predicate(NNN)}, with induced edges. Equivalent mental
model: add a virtual root that depends on every matching issue, expand, then drop the
root. `deps <id>` is the singleton case of this (`predicate = id == NNN`).

The predicate should reuse the existing `list` filter vocabulary (`--status`, `--kind`,
`--field`, `--match`, etc.) rather than inventing a new one.

## Acceptance criteria
- [ ] `deps` accepts a seed predicate built from the existing `list`-style filters; the
      rendered graph is the union of each matching seed's directed cone.
- [ ] Result is deterministic and order-independent (nodes deduped, edges unioned).
- [ ] A seed that matches but has no edges renders as a singleton node.
- [ ] `deps <id>` remains the singleton special case (no behavior change).
- [ ] The flag is named so it is NOT confused with display-side pruning (#056) — this
      selects *where expansion starts*, it is not a node filter (see Notes).
- [ ] Tests cover: single-seed equals old `deps <id>`; multi-seed union dedups shared
      nodes and unions edges; overlapping cones collapse; an isolated seed shows alone.

## Notes
Design context (from discussion):

- **Seed semantics, NOT display filter — keep them distinct.** This predicate picks
  where traversal *starts*; expansion then freely pulls in nodes that do *not* match the
  predicate (anything downstream of a seed). That is the correct generalization of
  `deps <id>`. It is deliberately different from "show only issues matching p" — e.g. a
  status-based seed shows matching issues *plus their dependencies*, not only matching
  nodes. Display-side pruning lives in #056. Name the two features so they cannot be
  mistaken for each other (avoid a bare `--filter`).
- **Direction** is inherited from `deps <id>` (whatever cone direction it already walks);
  the union introduces no new ambiguity.
- Touchpoints: the `deps` command handler / Graph view scoping added in #046/#047, and
  the shared `list`-filter predicate builder.
