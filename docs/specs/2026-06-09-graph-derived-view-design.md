# Graph: a derived read view over the loaded index

Status: accepted (2026-06-09)

## Problem

Every read command rebuilds the same derived structures over the flat `list[Issue]`
returned by `load_index`, then filters/sorts/formats its own way. The duplicated
derivations are:

| Derivation | Rebuilt in |
| --- | --- |
| `by_id = {r.id: r}` | `cmd_list`, `cmd_ready`, `cmd_deps` |
| `reverse` (dep id → dependents) | `cmd_list`, `cmd_deps` |
| `kids_of` / children map | `cmd_tree`, `generate_summary` |
| `parent_ids` set | `cmd_ready`, `normalize_points` |
| `is_blocked` / `is_terminal` / leaf checks | scattered, recomputed per call |

The processing each command does "to figure out what to print" is mostly this graph
reconstruction plus a handful of predicates, not the rendering itself. The four output
shapes (`list` flat rows, `tree`/`deps` recursive walks, `summary` rollups) are
genuinely different, so the thing to unify is the **graph**, not the printing.

## Design

Introduce a small read-only `Graph` value object built once per command from
`(cfg, rows)`. `rows` stays the single source of truth; `Graph` is a derived view and
is never mutated. No back-references are bolted onto `Issue` — the view stays separate
from the rows it indexes.

### Placement

A new "issue graph" band immediately after the index-I/O band (after `get_row`). It
depends only on `Issue` and the `cfg` helpers (`is_terminal`, `status_role`), all
defined above it.

### Class shape

```python
class Graph:
    """Derived read view over a loaded index: id lookup, parent/child and
    depends_on edges, and the readiness/blocking predicates. Built once per
    command from (cfg, rows); never mutated."""
    def __init__(self, cfg, rows):
        self.cfg, self.rows = cfg, rows
        self.by_id = {r.id: r for r in rows}
        self._children, self._dependents = {}, {}
        for r in rows:
            if r.parent is not None:
                self._children.setdefault(r.parent, []).append(r)
            for d in r.depends_on:
                self._dependents.setdefault(d, []).append(r)
        self._parents = set(self._children)        # ids with >= 1 child
```

A thin loader parallels `load_index`:

```python
def load_graph(ctx) -> Graph:
    return Graph(ctx.cfg, load_index(ctx))
```

### Method surface

| Method | Returns | Replaces |
| --- | --- | --- |
| `row(id)` | `Issue` (dies if missing) | `get_row` |
| `get(id)` | `Issue \| None` | `by_id.get` |
| `children_of(r)` | `[Issue]` sorted by id | `kids_of` / `children` closures |
| `dependents_of(r)` | `[Issue]` sorted by id | `reverse` map + `blocks` closure |
| `requires_of(r)` | `[Issue]` for sorted `depends_on` | `requires` closure |
| `is_terminal(r)` | bool | `is_terminal(cfg, r.status)` |
| `is_blocked(r)` | bool | `is_blocked(cfg, r, by_id)` |
| `is_leaf(r)` | `r.id not in self._parents` | `parent_ids` + membership test |
| `is_ready(r)` | `not terminal and leaf and not blocked` | the `is_ready` closure |
| `cycles()` | `[[int]]` | `find_dep_cycles(by_id)` |
| `would_cycle(src, dep)` | bool | `dep_would_cycle(by_id, …)` |

Sorting is baked into the accessors (`children_of`/`dependents_of`/`requires_of` return
id-sorted lists) so no caller re-sorts.

### Call-site migration

- `cmd_ready` / `cmd_next`: the setup + `is_ready` closure collapse to
  `r for r in g.rows if g.is_ready(r)`.
- `cmd_deps`: `by_id`, the `reverse` map, and the `requires` / `blocks` closures vanish;
  `walk_tree`'s `children_fn` becomes `g.requires_of` / `g.dependents_of` directly.
- `cmd_list`: `by_id` and the hand-rolled `reverse` loop go; `keep()` calls
  `g.is_blocked(r)`. `block_annotations` changes signature from
  `(ctx, r, by_id, reverse)` to `(g, r)` and reads `g.requires_of` / `g.dependents_of`.
- `cmd_tree`: the `kids_of` map and `children` closure become `g.children_of`.

### Decision: absorb the free graph functions

`find_dep_cycles`, `dep_would_cycle`, and `parent_ids` become `Graph` methods
(`cycles()`, `would_cycle()`, `is_leaf`) and the standalone functions are deleted;
`validate` and `cmd_dep` build a `Graph` and call the methods. This gives the graph one
home. It is the larger, test-heavier change, sequenced as its own step (Phase 1b) so it
lands independently.

## Scope and sequencing (interleave with #031)

The merged-browse work (#031: "Merge tree into list") rewrites exactly the hot spots a
naive Graph migration would touch (`cmd_list`, `cmd_tree`, `walk_tree`, `print_rows`,
`block_annotations`) and introduces new graph traversal (ancestor closure, match-set
propagation, sibling-recursive sort) that is itself a `Graph` responsibility. To avoid
writing `cmd_list`/`cmd_tree` twice, the Graph lands **first but scoped** — it does not
migrate `list`/`tree`. #031 then builds the merged verb directly on the Graph,
extending the class with the traversal it needs.

Single linear chain (each phase depends on the prior; `depends_on` = must be done
first):

- **Phase 0** — Graph substrate: class + `load_graph` + accessors + predicates + tests.
  No call-site changes. *(Graph epic)*
- **Phase 1** — migrate `ready`/`next` and `deps` to the Graph view. Output identical.
  Validates the API on simple, non-`list`/`tree` callers. *(Graph epic)*
- **Phase 1b** — absorb `find_dep_cycles` / `dep_would_cycle` / `parent_ids` into the
  class; migrate `validate` and `dep`. *(Graph epic)*
- **Phase 2** — extract the shared row renderer with a connector `prefix` (pass `""`
  everywhere; flat output byte-identical). A no-op refactor consumed by Phase 4.
  *(#031)*
- **Phase 3** — add `ancestors_of` + match-closure + sibling-sort traversal to `Graph`,
  unit-tested in isolation, no command wiring. Demand-driven by #031, so it is a #031
  child even though the code lands in the `Graph` class. *(#031)*
- **Phase 4** — implement nested `list` once: forest by default, `--flat`, positional
  `<id>`, filter = match set + dimmed ancestor spine, `--sort` reorders siblings
  recursively, `tree` → alias, `block_annotations(g, r)` through both paths. *(#031)*
- **Phase 5** — argparse/help/README + acceptance tests. *(#031)*

### Why this is least-waste

- Phases 0–1 never touch `list`/`tree`, so #031 never rewrites Graph-migrated code.
- Phase 2's renderer is exactly #031's "one shared renderer"; built once, consumed in 4.
- Phase 3 puts ancestor-closure in the class, so #031 does not hand-roll `kids_of` +
  closure inline (the duplication this whole effort sets out to kill).
- Each phase is an independent green commit; stopping after Phase 1 still ships value.

The chain is intentionally strict-linear even though Phase 1b and the Phase 2 / Phase 3
pair are independent in principle — kept serial for a predictable single-threaded
implementation order.

## Testing

- Phase 0: unit-test the `Graph` class directly — map construction, each accessor's
  sort order, each predicate against a small fixture index (including a blocked leaf, a
  parent, an orphan, a terminal blocker that clears a block).
- Phases 1/1b: existing command tests stay green (output unchanged); add coverage for
  `cycles()` / `would_cycle()` parity with the old free functions.
- Phases 2–5: covered under #031's acceptance criteria.

## Non-goals

- No change to the on-disk index format or `Issue` shape.
- No unification of the four output renderers — only the graph derivations are shared.
- `ready`/`next` stay a distinct work-queue query; not reframed as `list` presets.
