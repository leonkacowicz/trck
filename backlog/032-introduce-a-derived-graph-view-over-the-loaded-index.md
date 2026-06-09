# Introduce a derived Graph view over the loaded index

## Summary

Every read command rebuilds the same derived structures over the flat `list[Issue]`
(`by_id`, the reverse-dependency map, the children map, the `parent_ids` set) and
recomputes the `is_blocked`/`is_terminal`/leaf predicates inline. This epic introduces a
small read-only `Graph` value object, built once per command from `(cfg, rows)`, that
owns those derivations and predicates so commands query it instead of reconstructing it.

Full design — class shape, method surface, call-site migration, and the interleave with
#031 — lives in the spec: `docs/specs/2026-06-09-graph-derived-view-design.md`.

Scoped deliberately: this epic lands the substrate and migrates the simple callers
(`ready`/`next`, `deps`, `validate`, `dep`). It does **not** touch `list`/`tree` — #031
builds the merged browse verb directly on the Graph, so those hot spots are written once.

## Acceptance criteria
- [ ] A `Graph` class + `load_graph` exist as their own band after index I/O (#033).
- [ ] `ready`/`next` and `deps` query the Graph; output is unchanged (#034).
- [ ] The standalone graph functions (`find_dep_cycles`/`dep_would_cycle`/`parent_ids`)
      are absorbed into `Graph`; `validate` and `dep` use the methods (#035).
- [ ] `trck check` passes; all existing tests stay green.

## Notes

Children are a strict-linear chain: #033 → #034 → #035, then the seam into #031 at #036.
This epic is done exactly when #033–#035 are done.
