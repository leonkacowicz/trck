# list: order siblings by a global topological rank (blockers first)

## Summary
`list` orders siblings by `--sort` alone (created, then id, by default), so a blocker can render
below the issue it blocks. `deps` already draws a topological order; the nested forest should agree
with it, so reading `list` top-to-bottom is reading a plausible order of work.

The change is a different comparator, not a different walk. `cmd_list` builds one `sorted` closure
and hands it to the roots, every sibling group in `walk`, the JSON walk, and the flat/paths modes,
so replacing it lands everywhere at once.

**The design: one global topological rank, projected onto each sibling group.**

1. Kahn over the whole graph — authored dependencies *plus* the inferred "parent needs child" edge
   `deps` already infers — producing an `id -> usize` rank map computed once.
2. Sibling order is that rank. Nothing else changes; the tree walk is untouched.

Two properties this buys over a Kahn run per sibling group:

- **Transitivity for free.** Siblings X and Y with no edge between them, where X blocks Z in another
  epic and Z blocks Y: a global order forces X before Y. A per-group sort over direct sibling edges
  cannot see that.
- **Filter stability.** The rank is a lookup, computed over the whole graph, so a filtered `list`
  shows survivors in the same relative order as the unfiltered one. A Kahn computed over only the
  *shown* siblings reshuffles when a filter drops a node.

**A parent ranks by the minimum rank in its subtree, not by its own.** With the inferred edges, a
parent's own rank is necessarily after everything it contains — so in a mixed sibling group (one
leaf, one sub-epic) the sub-epic would be held back below the leaf even with **zero dependencies
authored anywhere**, which is an ordering change for trackers that use no dependencies at all.
Taking the subtree minimum means an epic sits where its work *starts* rather than where it
*finishes*: it interleaves with leaf siblings by `--sort` as it does today, and a ready epic stays
near the top instead of sinking to wherever its most-blocked child lands.

**The ready set is popped by the existing `sort_key`, not by a stack.** `gutter::topo` uses a LIFO
for depth-first lane locality, which is right for drawing a diagram and wrong here — it would let
branch structure decide sibling order. A min-heap keyed on `sort_key` keeps
`--sort created|priority|points|id|field:NAME` meaningful as the tie-break among mutually
unconstrained nodes, and keeps the no-dependency case identical to today's output.

`shorten_lanes` is not reused: it is a lane-drawing heuristic, and tree order must not depend on how
a diagram wants to look.

## Acceptance criteria
- [ ] A topological rank map over the whole graph — authored deps plus inferred parent-needs-child —
      built once per `cmd_list` invocation, with the ready set popped by the current `sort_key`.
- [ ] No `shorten_lanes` (or any other lane heuristic) in the path that produces the rank.
- [ ] A parent's effective rank is the minimum rank over its subtree, including itself.
- [ ] The `sorted` closure in `cmd_list` sorts by that rank, so the forest, `--json`, `--flat` and
      `--paths` all inherit it; `walk`/`json_node` are unchanged.
- [ ] Cycles degrade rather than hang or drop rows: the Kahn residue is appended in `sort_key`
      order. (`check` only *warns* about effective cycles — `Graph::effective_cycles` — and parent
      cycles are possible too, so `list` must render a malformed tracker.)
- [ ] `--sort` still selects the tie-break among unconstrained siblings; a tracker with no
      dependencies produces byte-identical output to today, in mixed leaf/parent sibling groups
      included.
- [ ] Conformance fixtures: a sibling pair ordered by an authored dep against `--sort created`; the
      transitive case (constraint routed through another subtree); a filtered view keeping the
      unfiltered relative order; a cyclic tracker still rendering.
- [ ] `ratchet generate` run and the regenerated `quality-report.json` staged with the change.

## Notes
- Touch points: `sort_key` and the `sorted` closure at `src/query/list.rs:35` and `:167`; the walks
  that consume it at `:225`, `:244`, `:297`. `gutter::topo` at `src/gutter/mod.rs:277` is the Kahn
  to model this on (and `:188`, where components are seeded by lowest id — here the seed is
  `sort_key` instead).
- Effective/lifted dependency helpers already exist: `Graph::lifted_deps` (`src/graph.rs:140`),
  `requires_of`, `dependents_of`, `effective_cycles` (`:422`), `parent_cycles` (`:436`).
- Blast radius in the conformance suite is small: exactly one fixture combines a `list`/`tree`
  command with any dependency edges.
- **Open follow-up:** `SUMMARY.md` (`src/summary.rs:131`, priority then id) and the HTML tree
  (`src/html.rs:139`) sort independently, so they would start disagreeing with `list`. Decide
  whether they follow — likely a separate issue once this lands.
- Not the same as [[deps-order-graph-components-by-demand-most-demanded-cluster-first]] (`#j967byf`),
  which orders *components* in the `deps` gutter by demand. Independent; no edge between them.
