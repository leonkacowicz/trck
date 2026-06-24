# deps: --depth N to cut expansion N hops from the seed(s)

## Summary
Add a `--depth N` traversal limit to the `deps` graph: render only nodes within N hops
of a seed. This is a property of the *traversal*, distinct from both seed selection
(#hzy98pm) and the done-filtering display flags (#dbq2wqn) — it bounds *how far* expansion
reaches, not *where* it starts or *which* nodes are pruned afterwards.

Split out of #dbq2wqn, which now focuses solely on done-filtering.

## Acceptance criteria
- [ ] `--depth N` limits rendering to nodes within N hops of a seed.
- [ ] Composes with the multi-seed union from #hzy98pm (depth measured from the nearest seed).
- [ ] Composes with the directional scoping already in `deps` (`--requires`/`--blocks`):
      depth counts hops along whichever cone(s) are in scope.
- [ ] Read-only/display-only: never mutates issues or the index.
- [ ] Tests cover: depth truncates at the right hop count; composition with a multi-seed
      graph; composition with `--requires`/`--blocks`.

## Notes
- Touchpoints: the Graph render/scoping path (#chzay3q / #bt9pwy8) and `_print_deps_graph`.
- Open question: is `--depth` only meaningful with an explicit seed/id, or should it also
  bound the full-graph (no-id) view? Likely seed-scoped only — N hops from *what* is
  undefined without a seed.
