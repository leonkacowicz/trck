# trck-html: mirror demand-cone ranking in the ready/next view

## Summary
`ready`/`next` in the CLI rank by the **demand cone** (#9bktptp): an issue plus every
unfinished issue transitively waiting on it, so a medium task standing between us and an
urgent one outranks a high one that blocks nothing. `tools/trck-html` has no ready view at
all — it exports `ready`/`blocked` per issue and never uses them — so the same tracker
tells you two different stories about what to pick up next.

Add **`ready`** as a fifth view beside list/graph/tree/board, ranked exactly as the CLI
ranks it, with the same `↑<priority>(#culprit)` marker.

## Acceptance criteria
- [x] The model exports engine-computed `demand` (the priority-count vector) and
      `demand_source` per issue — no cone math is re-derived in JS
- [x] A `ready` view button and pane exist; the view lists only actionable leaves
      (`is_ready`) and honours the search / status / priority filters like every other view
- [x] Rows sort by the demand vector slot-by-slot descending, then `-points`, then `id` —
      the same key as `cmd_ready`
- [x] A row whose cone outranks its own priority is marked `↑<priority>(#culprit)`,
      coloured as that priority
- [x] The top row is marked as the `next` pick (`trck next` is `ready[:1]`)
- [x] Tests cover the exported fields and the presence of the ready UI

## Notes
**Where the ranking lives.** The sort key is authored once in the engine
(`Graph.demand_vector` / `demand_source`) and shipped as data; the client only compares
vectors. That keeps trck-html's coupling to the engine's *values*, not its algorithm — if
the cone definition changes, the page follows without a JS edit.

**`demand_source` is exactly the marker condition.** The engine returns `None` when the
row is already its cone's maximum priority, so the client can render the marker iff the
field is set — no rank comparison in JS.

**Out of scope.** The list view keeps its declared-priority sort, mirroring the CLI, where
`list --sort priority` deliberately sorts the stored field (#9bktptp). Scoping ready to a
subtree (`trck ready ID`) has no UI equivalent here; the filter bar covers the need.
