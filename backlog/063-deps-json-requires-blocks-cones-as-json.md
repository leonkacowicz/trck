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
Depends on #060. Handler `cmd_deps` ~line 1666; it derives `up`/`down` and calls
`_print_deps_graph` (~line 1625). The cone walk lives in the `Graph` (predecessors
= requires, successors = blocks). Open question to resolve in this issue: the
no-id (whole graph) and `--full` (whole connected cluster) cases — simplest first
cut is to support the id form (`requires`/`blocks` cones) and define no-id
explicitly rather than silently emitting nothing.
