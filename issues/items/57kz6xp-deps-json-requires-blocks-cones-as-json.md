# deps --json: {requires, blocks} cones as JSON

## Summary
Emit an issue's dependency relationships as JSON instead of the lazygit-style
gutter. For a given id, `requires` is its prerequisite cone (what it depends on)
and `blocks` is its dependent cone (what waits on it) — the same two directions
`deps` already computes for the human graph.

- `deps NNN --json` → `{ "requires": [...], "blocks": [...] }`.

## Acceptance criteria
- [ ] `deps NNN --json` emits `{requires: [...], blocks: [...]}`; each entry is an issue object (`to_dict()`, or at least id+title+status — settle and document).
- [ ] `--requires` / `--blocks` scope the output to that single direction (the other key omitted or empty — pick and document).
- [ ] Honours the same cone computation as the human render (directed dependency line; `--full` semantics settled — see notes).
- [ ] Whole-graph `deps --json` with no id: decide and document (e.g. emit all edges, or require an id like `--requires/--blocks` do).
- [ ] One valid JSON document via the #060 helper; default human graph unchanged.
- [ ] Field shape documented in `deps` help; tests assert parseable JSON + both cones.

## Notes
Depends on #060. Handler `cmd_deps` — `src/trck/cmd_query.py:339`; it derives
`up`/`down` and calls `_print_deps_graph` (`src/trck/cmd_query.py:287`).

The cone walk is `Graph.dependency_line(row, up=…, down=…)` —
`src/trck/graph.py:272` — which returns one id set covering both sweeps, so for
`{requires, blocks}` call it once per direction (`up=True, down=False` and the
reverse) and drop the focal id from each. Both sweeps follow *drawn* edges
(`drawn_deps_of` / `drawn_dependents_of` — `src/trck/graph.py:72` / `:89`), so
inferred parent↔child containment edges are in the cones too: decide and document
whether the JSON marks those as inferred (the human gutter dims them).

Open question to resolve in this issue: the no-id (whole graph) and `--full`
(whole connected cluster) cases — no-id uses `deps_overview_ids`
(`src/trck/render.py:428`), `--full` uses `graph_components`
(`src/trck/render.py:251`). Simplest first cut is to support the id form
(`requires`/`blocks` cones) and define no-id explicitly rather than silently
emitting nothing.
