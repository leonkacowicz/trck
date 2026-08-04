# list --json: nested hierarchy (and --flat --json flat array)

## Summary
Emit `list` results as JSON. Default (nested) mirrors the on-screen forest:
top-level issues with their children nested under them. `--flat --json` emits a
flat, globally-sorted array of the matched rows. Both honour every existing
filter/sort exactly as the human render does.

- `list --json` → array of root objects, each `{...to_dict(), "children": [...]}` (recursive).
- `list --flat --json` → flat array of `to_dict()` objects in the sorted order.

## Acceptance criteria
- [x] `list --json` produces the nested forest as JSON; each node is `to_dict()` plus a `children` array (empty when none).
- [x] `list --flat --json` produces a flat array in the same order as `--flat` human output.
- [x] All existing filters (`--status/--kind/--priority/--label/--parent/--match/--field/--blocked/--orphan`), `--sort`, and the optional root `id` are honoured; empty result → `[]`.
- [x] Nested shape reuses the existing forest layout (`match_closure`/`forest_layout`); dimmed ancestor-context rows are included (they appear in the forest).
- [x] Output is one valid JSON document via the #v8tmkrt helper; default human output unchanged.
- [x] Field shape documented in `list` help; tests assert parseable JSON + nesting + filter honouring.

## Notes
Depends on #v8tmkrt (emit_json + `--json` flag). Handler `cmd_list` —
`src/trck/cmd_query.py:68`; `forest_layout` — `src/trck/cmd_query.py:44`;
`Graph.match_closure` — `src/trck/graph.py:112`, called at `cmd_query.py:155` —
builds `shown`/`dim`. For the nested form, build the child lists from the same
`shown` set and sibling `key` ordering the human render uses, so JSON and screen
agree. Decide and document whether
dimmed context rows carry a marker (lean: include them as normal nodes; consumers
filter by status if they want only matches).

## Decided while building
**`context` is a new field, not in the plan.** The forest pulls non-matching ancestors in
so a matched child never floats free, and the human view distinguishes them by dimming.
The criterion said only "include them" — but included and unmarked, a consumer filtering
by `--match` cannot tell a result from the scaffolding holding it. The information is on
screen; leaving it out of the data would have been a silent loss.

**`--paths --json` is refused** rather than letting `--paths` quietly win, which is what
the existing early return would have done.
