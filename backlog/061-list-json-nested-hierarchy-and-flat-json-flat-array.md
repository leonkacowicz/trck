# list --json: nested hierarchy (and --flat --json flat array)

## Summary
Emit `list` results as JSON. Default (nested) mirrors the on-screen forest:
top-level issues with their children nested under them. `--flat --json` emits a
flat, globally-sorted array of the matched rows. Both honour every existing
filter/sort exactly as the human render does.

- `list --json` → array of root objects, each `{...to_dict(), "children": [...]}` (recursive).
- `list --flat --json` → flat array of `to_dict()` objects in the sorted order.

## Acceptance criteria
- [ ] `list --json` produces the nested forest as JSON; each node is `to_dict()` plus a `children` array (empty when none).
- [ ] `list --flat --json` produces a flat array in the same order as `--flat` human output.
- [ ] All existing filters (`--status/--kind/--priority/--label/--parent/--match/--field/--blocked/--orphan`), `--sort`, and the optional root `id` are honoured; empty result → `[]`.
- [ ] Nested shape reuses the existing forest layout (`match_closure`/`forest_layout`); dimmed ancestor-context rows are included (they appear in the forest).
- [ ] Output is one valid JSON document via the #060 helper; default human output unchanged.
- [ ] Field shape documented in `list` help; tests assert parseable JSON + nesting + filter honouring.

## Notes
Depends on #060 (emit_json + `--json` flag). Handler `cmd_list` ~line 1455;
`forest_layout` ~line 1431, `match_closure` builds `shown`/`dim`. For the nested
form, build the child lists from the same `shown` set and sibling `key` ordering
the human render uses, so JSON and screen agree. Decide and document whether
dimmed context rows carry a marker (lean: include them as normal nodes; consumers
filter by status if they want only matches).
