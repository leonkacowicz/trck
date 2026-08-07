# list: order siblings by a global topological rank (blockers first)

## Summary
`list` ordered siblings by `--sort` alone (created, then id, by default), so a blocker could render
below the issue it blocks. `deps` already draws a topological order; the nested forest now agrees
with it, so reading `list` top-to-bottom is reading a plausible order of work.

The change is a different comparator, not a different walk. `cmd_list` builds one `sorted` closure
and hands it to the roots, every sibling group in `walk`, the JSON walk, and the flat/paths modes,
so replacing it landed everywhere at once.

**The design: one global topological rank, projected onto each sibling group.**

1. Kahn over the whole graph, producing an `id -> usize` rank map computed once.
2. Sibling order is that rank, with `--sort` breaking ties. Nothing else changed; the tree walk is
   untouched.

Two properties this buys over a Kahn run per sibling group:

- **Transitivity for free.** Siblings X and Y with no edge between them, where X blocks Z in another
  epic and Z blocks Y: a global order forces X before Y. A per-group sort over direct sibling edges
  cannot see that.
- **Filter stability.** The rank is a lookup, computed over the whole graph, so a filtered `list`
  shows survivors in the same relative order as the unfiltered one. A Kahn computed over only the
  *shown* siblings reshuffles when a filter drops a node.

**The constraint set is the effective dependency relation, and containment is not an edge.** The
source side is `lifted_deps` (an edge authored on a parent holds down its whole subtree); the target
side expands to `subtree` (waiting on an epic waits for everything inside it). This is the relation
`check` already walks for effective cycles.

The plan had been to include the inferred *parent needs child* edge that `deps` draws, and rank a
parent at its subtree's minimum to stop that edge from sinking every epic below its own children.
Measurement killed it: that edge puts a parent after everything it contains, so an epic filed months
before its first child sinks to the child's position — **23 of 229 rows moved on this repo's own
tracker with every dependency stripped out.** No later adjustment recovers the parent's own position
once the edge is in. Dropping the edge and rating a parent at `min(its own rank, its subtree's)`
gives both: byte-identical output when nothing is blocked, and an epic that leads with the earliest
row it holds. The cost, accepted deliberately: an epic whose children are all blocked can still sit
above their blocker, because its own row still counts.

**A done dependency constrains nothing.** Same rule as `is_blocked`, and the same one that clears a
row's `needs #NNN` note — an order still reflecting a satisfied dependency would contradict the note
printed beside it.

**The ready set is popped by `sort_key`, not by a stack.** `gutter::topo` uses a LIFO for depth-first
lane locality, which is right for drawing a diagram and wrong here — it would let branch structure
decide sibling order. A min-heap keyed on `sort_key` keeps `--sort created|priority|points|id|
field:NAME` meaningful among mutually unconstrained rows. `shorten_lanes` is not reused either: tree
order must not depend on how a diagram wants to look.

## Acceptance criteria
- [x] A topological rank map over the whole graph, built once per `cmd_list` invocation, with the
      ready set popped by the current `sort_key`.
- [x] Constraints are the effective dependency relation — `lifted_deps` on the source side, `subtree`
      on the target side — with containment deliberately *not* an edge, and terminal dependencies
      excluded.
- [x] No `shorten_lanes` (or any other lane heuristic) in the path that produces the rank.
- [x] A parent's effective rank is the minimum over its subtree, including itself.
- [x] The `sorted` closure in `cmd_list` sorts by that rank, so the forest, `--json`, `--flat` and
      `--paths` all inherit it; `walk`/`json_node` are unchanged.
- [x] Cycles degrade rather than hang or drop rows: the Kahn residue is appended in `sort_key`
      order. (`check` only *warns* about effective cycles — `Graph::effective_cycles` — and parent
      cycles are possible too, so `list` must render a malformed tracker.)
- [x] `--sort` still selects the tie-break among unconstrained siblings; a tracker with no open
      dependencies produces byte-identical output to before, mixed leaf/parent sibling groups
      included. Verified against this repo's tracker with `depends_on` stripped from every row.
- [x] Conformance fixtures: a sibling pair ordered by an authored dep against `--sort created`; the
      transitive case (constraint routed through another subtree); a filtered view keeping the
      unfiltered relative order; a done blocker no longer constraining; a cyclic tracker still
      rendering. `--min-pass` raised 237 → 242.
- [x] `ratchet generate` run and the regenerated `quality-report.json` staged with the change.

## Notes
- The rank lives in `src/query/rank.rs`, alongside `sort_key`/`seed_key`, which moved there from
  `list.rs` — the two are the same question (what order do rows come out in), and the move is what
  paid for the new code under the quality ratchet.
- `src/test_graph.rs` is new: the compact `graph(&["a", "b:a ->c"])` builder that was private to
  `graph.rs`'s test module, so `rank.rs` can use it without a second copy.
- Effective/lifted dependency helpers were already there: `Graph::lifted_deps`, `subtree`,
  `effective_cycles`, `parent_cycles`.
- Only one existing fixture changed: `list-json-flat-is-a-flat-array`, whose blocker now precedes
  the issue waiting on it.
- On this repo's tracker the whole visible effect is two rows: `#wtmfdhr` now renders above
  `#6xcseef`, which needs it. 113 authored edges, no open blocker listed after its dependent.
- **Open follow-up:** `SUMMARY.md` (`src/summary.rs`, priority then id) and the HTML tree
  (`src/html.rs`) sort independently, so they now disagree with `list`. Decide whether they follow —
  a separate issue.
- Not the same as [[deps-order-graph-components-by-demand-most-demanded-cluster-first]] (`#j967byf`),
  which orders *components* in the `deps` gutter by demand. Independent; no edge between them.
