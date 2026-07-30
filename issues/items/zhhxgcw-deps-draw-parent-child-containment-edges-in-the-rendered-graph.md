# deps: draw parent->child containment edges in the rendered graph

## Summary

Feed a synthetic `parent -> child` edge into the deps graph for every parent/child pair, so
a parent's dependency cone includes its own subtree. This is the single change that makes
`trck deps <epic>` answer "what is needed to complete this epic": the forward cone becomes
the open descendants **plus** their external prerequisites, topologically ordered.

Justified by the model, not invented for the view: a parent is done exactly when all its
children are, which is precisely "the parent depends on its children".

## Acceptance criteria
- [ ] The rendered graph includes `parent -> child` edges for every parent/child pair in
      the id set.
- [ ] `trck deps <parent>` no longer prints "(no dependencies)" for a parent whose children
      carry edges; `--requires` on it yields subtree ∪ external prerequisites.
- [ ] Containment edges are visually distinct from authored edges.
- [ ] The unscoped whole-graph view stays usable — see the seeding note below.
- [ ] `index.jsonl` unchanged; the edges exist only in the render path.

## Notes

- Touch points: `graph_components` (`trck:1189`), `_graph_topo` (`trck:1214`) and
  `Graph.dependency_line` (`trck:687`) all read `r.depends_on` directly. Prefer introducing
  **one** edge accessor on `Graph` that yields authored + inferred edges and routing all
  three through it, over patching each site.
- **Seeding is the open question.** `_print_deps_graph` builds `edged` = every issue
  touching an edge. With containment edges that becomes *every issue with a parent or a
  child*, i.e. most of the tracker, and the unscoped `trck deps` stops being a dependency
  view. Options: keep the unscoped view on authored edges only and switch containment on
  for the scoped `deps ID` view; or gate it behind a flag. Decide before implementing.
- Rendering: the lane system is one-row-per-node with `_GRAPH_GLYPH` mapping a
  `{U,D,L,R}` set to a box-drawing char (`trck:1177`). A distinct containment glyph set
  (dashed `╎`/`┄`) needs a parallel table; a distinct *colour* rides on the existing
  `owners`/`paint_lane` mechanism and is much cheaper. Try colour first.
- Expect this to make the graph noticeably denser on its own. #atk4umk (transitive
  reduction) is what pays that back; they are best evaluated together even though this one
  lands first.
