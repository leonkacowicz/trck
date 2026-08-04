# ready/next --json: ranked actionable leaves with demand annotations

## Summary
Emit the `ready`/`next` result as JSON instead of the coloured rows. This is the
verb an automated consumer actually wants: it already answers "what should be
worked on now, in what order", so a scripted or agent-driven loop reduces to
`next --json` → do the work → `done` → repeat. Today that loop has to scrape
rendered rows carrying ANSI colour, abbreviated ids, and `↑urgent(#id)` markers.

- `ready --json` → a JSON array in rank order.
- `next --json` → the same, capped at one (`ready --next` is the same code path).

The **rank order is the payload**. Callers must not need to re-derive it, so the
order of the array is the contract, and the demand annotation each row displays
should be carried as data rather than baked into a string.

## Acceptance criteria
- [x] `ready --json` emits one valid JSON document: an array of issue objects in
      the exact order the human render uses.
- [x] `next --json` emits the same shape truncated to one entry (array, not a bare
      object — the same shape either way keeps consumers simple).
- [x] Each entry carries the demand annotation as fields, not prose: the inferred
      priority and the driving issue id (what `demand_annotation` renders as
      `↑urgent(#a1b2c3)`), omitted when a row isn't lifted above its own priority.
- [x] `ready ID --json` honours subtree scoping, filtering the *result* exactly as
      the human path does — readiness and ranking stay computed over the whole graph.
- [x] Ids are emitted in full, never abbreviated — `unique_prefix_lens` is a display
      concern and must not leak into the data.
- [x] Empty result emits `[]`, not nothing, and exits 0.
- [x] Goes through the shared emit seam from #v8tmkrt; default human output unchanged.
- [x] Documented in `ready`/`next` help; tests assert parseable JSON, the rank order,
      and the lifted-priority fields.

## Notes
Depends on #v8tmkrt for the `--json` flag plumbing and the stdout seam.

Handler is `cmd_ready` — `src/trck/cmd_query.py:308` — with `cmd_next` delegating
to it via `ns_like(args, next=True)` (`:333`), so a single `--json` branch covers
both verbs. The sort key is already computed there:
`(*(-n for n in g.demand_vector(r)), -r.points, r.id)`; serialise `rows` in that
order rather than re-sorting downstream.

The annotation fields come from `Graph.demand_source` (`src/trck/graph.py:256`),
which returns the issue driving the lift, alongside `demand_vector`
(`src/trck/graph.py:243`) — the same pair `demand_annotation` formats for the
human row. Settle whether an entry is the full `to_dict()` or a narrower
id+title+status+priority shape, and match whatever #k9snjz3 lands on for `list`,
so the two verbs don't disagree about what an issue looks like in JSON.

Unblocks the unattended-loop frame of the README demo recording (#jkvexgs).
