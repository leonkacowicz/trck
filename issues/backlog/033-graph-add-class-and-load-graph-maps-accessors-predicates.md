# Graph: add class and load_graph (maps, accessors, predicates)

## Summary

Phase 0 of the Graph epic (#032). Pure addition — no command changes, no behavior
change. Add the `Graph` class and a `load_graph(ctx)` loader in a new "issue graph" band
right after the index-I/O band (after `get_row`), depending only on `Issue` and the
`cfg` helpers above it.

`Graph.__init__(cfg, rows)` builds `by_id`, the children map, the reverse-dependency
map, and the `_parents` set once. Accessors return id-sorted lists; predicates wrap the
existing logic. See the spec for the exact surface:
`docs/specs/2026-06-09-graph-derived-view-design.md`.

## Acceptance criteria
- [ ] `Graph` class added: `by_id`, children, dependents, and `_parents` built in `__init__`.
- [ ] Accessors `row`, `get`, `children_of`, `dependents_of`, `requires_of` — the three
      list accessors return id-sorted lists.
- [ ] Predicates `is_terminal`, `is_blocked`, `is_leaf`, `is_ready`.
- [ ] `load_graph(ctx)` parallels `load_index`.
- [ ] Unit tests cover map construction, accessor sort order, and each predicate against a
      fixture index (blocked leaf, parent, orphan, terminal blocker that clears a block).
- [ ] No existing command is modified; all tests green; `trck check` passes.

## Notes

`rows` stays the source of truth; `Graph` is a derived view and is never mutated — no
back-references added to `Issue`.
