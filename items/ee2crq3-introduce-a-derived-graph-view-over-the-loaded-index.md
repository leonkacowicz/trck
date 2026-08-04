# Introduce a derived Graph view over the loaded index

## Summary

Every read command rebuilds the same derived structures over the flat `list[Issue]`
(`by_id`, the reverse-dependency map, the children map, the `parent_ids` set) and
recomputes the `is_blocked`/`is_terminal`/leaf predicates inline. This epic introduces a
small read-only `Graph` value object, built once per command from `(cfg, rows)`, that
owns those derivations and predicates so commands query it instead of reconstructing it.

Full design — class shape, method surface, call-site migration, and the interleave with
#qapvxpz — lives in the spec: `docs/specs/2026-06-09-graph-derived-view-design.md`.

Scoped deliberately: this epic lands the substrate and migrates the simple callers
(`ready`/`next`, `deps`, `validate`, `dep`). It does **not** touch `list`/`tree` — #qapvxpz
builds the merged browse verb directly on the Graph, so those hot spots are written once.

## Acceptance criteria
- [ ] A `Graph` class + `load_graph` exist as their own band after index I/O (#chzay3q).
- [ ] `ready`/`next` and `deps` query the Graph; output is unchanged (#bt9pwy8).
- [ ] The standalone graph functions (`find_dep_cycles`/`dep_would_cycle`/`parent_ids`)
      are absorbed into `Graph`; `validate` and `dep` use the methods (#n2gdhdd).
- [ ] `trck check` passes; all existing tests stay green.

## Notes

Children are a strict-linear chain: #chzay3q → #bt9pwy8 → #n2gdhdd, then the seam into #qapvxpz at #33frt7s.
This epic is done exactly when #chzay3q–#n2gdhdd are done.
