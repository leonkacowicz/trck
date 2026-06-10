# deps --graph <id>: scope to the focal issue's directed dependency line

## Summary
`deps --graph <id>` currently scopes to the *weakly-connected component* of the
issue. `graph_components` (`trck` ~L832) builds its adjacency with `depends_on`
edges treated as **undirected**, so the rendered set includes nodes that have no
directed dependency relation to the focal issue — they only happen to share an
ancestor or descendant with it.

Concrete case: A blocks B, A blocks C, B blocks D (so B, C both depend on A; D
depends on B). `deps --graph B` today shows A, B, C, D — but C is neither a
transitive prerequisite of B nor a transitive dependent of B. B and C are
"cousins" joined only through their shared prerequisite A.

Make `deps --graph <id>` show **only the focal issue's directed dependency
line**: the issue itself, everything it (transitively) depends on, and
everything that (transitively) depends on it. In the example,
`deps --graph B` should render A, B, D and omit C. Keep the current
weakly-connected-component view available behind a flag.

## Acceptance criteria
- [ ] `deps --graph <id>` renders exactly `{id} ∪ prerequisites*(id) ∪ dependents*(id)`,
      where `prerequisites*` is the forward closure along `depends_on` edges and
      `dependents*` the reverse closure. (NB: this is the **dependency** graph —
      `depends_on` — not the parent/epic spine that `Graph.ancestors_of` walks.)
- [ ] In the A→B, A→C, B→D example, `deps --graph B` shows A, B, D and omits C.
- [ ] The previous whole-component view stays reachable via a flag
      (e.g. `--component` / `--full`); decide the exact name at design time.
- [ ] The no-id whole-graph form (`deps --graph`) is unchanged — it has no focal
      node, so directed scoping doesn't apply.
- [ ] Reuses the existing `render_graph` / component-splitting renderer; the only
      new logic is computing the restricted id-set (a directed forward+reverse
      reachability over `depends_on`, similar to the traversal in
      `Graph.would_cycle`).
- [ ] Covered by tests (incl. the cousin-exclusion case); full suite and
      `trck check` green.

## Notes
Builds directly on #046 (the lazygit-style DAG renderer), which introduced
`graph_components`, `_graph_topo`, `_graph_component_rows`, `render_graph`, and
the `cmd_deps` graph branch. This issue changes *which* ids are passed into that
renderer when an `<id>` is given; the rendering path itself is untouched.

The user chose "compact view as the default" over adding a `--compact` opt-in:
the directed line is almost always what you want when you ask for one issue's
dependency graph, and the undirected component is the surprising part.
