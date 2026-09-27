## Summary

The page's graph view draws authored `depends_on` only: `build_model` (`src/html.rs`) ships
`requires_of` and nothing else, with a comment saying containment is deliberately left out.
`trck deps` stopped working that way in #dj5b42j — it adds containment (a parent waits on each
child) and inherited edges (a child waits on its ancestors' targets), then transitively
reduces. So the two views disagree about the same tracker.

What that looks like: an epic with no authored edges is missing from the page's graph
entirely, and an epic with one authored edge shows as a root with nothing feeding it. Seen on
a real tracker — `deps` drew an epic with eleven children fanning into it; the page drew the
same epic as a lone root with one outgoing edge, and its sub-epic not at all. "What is left
to finish this parent" has no answer on the page.

## Approach

The page's filters — include done chains, omit done, filter-bar seeding — run in the
browser, and both the inherited-edge suppression and the reduction depend on which ids are
on screen: an edge may only be dropped in favour of a path that is itself drawn. So the
engine cannot precompute the drawn set; the page derives it from what each issue row already
carries (`parent`, `children`, `requires`). A port of `src/gutter/edges.rs` and
`src/gutter/reduce.rs`, same rules:

- **Dep** — the issue's own `requires`. **Child** — one per child. **Inherited** — each
  ancestor's targets, nearest author first, dropped when a drawn ancestor between the issue
  and the author already carries it (`carried_above`). No `--fanout` on the page.
- **Reduction** over the visible set, after done-filtering.
- **Node selection** mirrors `gutter::overview_ids`: the components (over drawn edges) holding
  at least one authored edge, taken whole. **Filter-bar seeding** follows
  `Graph::dependency_line` — up is requires + children + lifted deps, down is dependents +
  parent.
- **Done-filtering** keeps its shape; its components are computed over drawn edges.

The model's `edges` array stays authored-only: it is the raw ingredient, not the drawing.

## Acceptance criteria

- [ ] On the same tracker, the page's default graph (done chains hidden, done omitted) draws
      the same nodes and edges as `trck deps --omit-done`.
- [ ] Containment and inherited edges are styled apart from authored ones.
- [ ] The derivation is covered in `tests/app_js.rs`, mirroring the gutter's unit tests.
- [ ] No inferred edge reaches `index.jsonl` or the model's `edges` array.
