# deps: infer hierarchy edges so the graph answers 'what is left to finish this parent'

## Summary

`trck deps` walks **authored `depends_on` only**. The parent hierarchy is invisible to it,
even though the hierarchy is load-bearing in the model. So the most natural question about
an epic — *what is needed to complete it?* — has no answer in the graph:

```
$ ./trck deps r9zefup
#r9zefup ○ Add --json output to list/show/deps/tree for scripting ·epic·  (no dependencies)
```

…while that epic's four children form a full chain. The renderer draws strictly less than
the engine knows.

Fix: derive two families of inferred edges from the hierarchy, render them alongside the
authored ones, then transitively reduce the result so the added edges don't drown the graph.

1. **Containment** — a parent depends on each of its children (a parent is done exactly
   when its children are).
2. **Inheritance** — a child depends on each of its ancestors' dependency targets (which
   are necessarily outside its own subtree).

**Soundness:** the combined relation is exactly the closure the engine already enforces.
`_eff_reach` composes `lifted_deps` (family 2) with `subtree` expansion of each target
(family 1), and `would_cycle` / the effective-cycle check in `validate` reject any authored
edge that would close a loop in it. So the combined graph is **already guaranteed acyclic**
— this introduces no new invariant. That guarantee is currently implicit; it should get an
explicit test.

Everything here is **display-only**. No inferred edge is ever written to `index.jsonl`.

## Acceptance criteria
- [ ] `trck deps <parent>` shows the parent's descendants and their external prerequisites,
      topologically ordered — blockers above what they block.
- [ ] Inferred edges are visually distinguishable from authored ones.
- [ ] The graph is transitively reduced by default; the default view keeps dependencies at
      the altitude they were authored.
- [ ] `--fanout` shows inherited dependencies expanded per child.
- [ ] `index.jsonl` is untouched by any of it.
- [ ] A test asserts the combined (authored + inferred) graph is acyclic for any tracker
      that passes `trck check`.

## Notes

- Renderer entry points: `_print_deps_graph` (`trck:2104`), `render_graph` (`trck:1330`),
  `graph_components` (`trck:1189`), `_graph_topo` (`trck:1214`) — all read `r.depends_on`
  directly today. `Graph.dependency_line` (`trck:687`) likewise seeds cones from authored
  edges only.
- Existing semantics to reuse rather than reimplement: `lifted_deps` (`trck:655`),
  `subtree`, `_eff_reach` (`trck:729`), `would_cycle` (`trck:744`).
- Ordering rationale between children: containment first (it alone answers the headline
  question); reduction next (it is what keeps the denser graph readable, and it shrinks
  lane count and bridge length, so it partly subsumes #xagqqgd — revisit that issue's scope
  once this lands); inheritance and the hoist last, since the hoist only has meaning once
  both exist.
- Deliberately **not** blocked on #9jgarpa. That bug gated an earlier design option — "draw
  containment only, let the `needs` annotation carry inheritance" — which we rejected in
  favour of drawing inheritance as real edges. The narrowest true edge here is no edge at all.
