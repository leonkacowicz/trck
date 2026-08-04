# deps --json: {requires, blocks} cones as JSON

## Summary
Emit an issue's dependency relationships as JSON instead of the lazygit-style
gutter. For a given id, `requires` is its prerequisite cone (what it depends on)
and `blocks` is its dependent cone (what waits on it) — the same two directions
`deps` already computes for the human graph.

- `deps NNN --json` → `{ "requires": [...], "blocks": [...] }`.

## Acceptance criteria
- [x] `deps NNN --json` emits `{requires: [...], blocks: [...]}`; each entry is an issue object (`to_dict()`, or at least id+title+status — settle and document).
- [x] `--requires` / `--blocks` scope the output to that single direction (the other key omitted or empty — pick and document).
- [x] Honours the same cone computation as the human render (directed dependency line; `--full` semantics settled — see notes).
- [x] Whole-graph `deps --json` with no id: decide and document (e.g. emit all edges, or require an id like `--requires/--blocks` do).
- [x] One valid JSON document via the #v8tmkrt helper; default human graph unchanged.
- [x] Field shape documented in `deps` help; tests assert parseable JSON + both cones.

## Notes
Depends on #v8tmkrt. Handler `cmd_deps` — `src/trck/cmd_query.py:339`; it derives
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

## Decided while building
**`--json` needs an id.** With none, the human graph draws every component — an edge
list, a different schema from a pair of cones. Emitting two shapes from one flag would
make every consumer branch on whether an id was passed, so the no-id case is refused with
a message saying why. A whole-graph edge export is a separate feature if wanted.

**Both keys always present**, `--requires`/`--blocks` empty the other rather than dropping
it, and the focal issue is excluded from its own cones (`dependency_line` includes it).

**Index order, not set order.** `dependency_line` returns a set, whose iteration order
varies with hash seeding. The conformance fixtures are the reason this exists, and no
golden file survives that.
