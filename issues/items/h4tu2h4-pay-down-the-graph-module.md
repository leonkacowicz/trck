# pay down the graph module

## Summary
With the gutter (`#nhscdux`) and `repo.rs` (`#9ttp6rn`) cleared, `src/graph/mod.rs` was the
worst file left: 146 total excess across four categories, 608 lines against a threshold of 300,
and 68 function spaces against 20.

Unlike the two before it, this file was not doing unrelated jobs. It was doing *one* job — the
derived view over a loaded index — at every altitude at once: construction, containment,
dependency lifting, the readiness predicates, the demand cone the ranking is built on, and the
points rollup. Read top to bottom it is a coherent essay; opened to answer a question it is 600
lines to search.

The module doc already named the organising idea — **lifting**, an authored edge inherited by
everything inside the source and satisfied only by everything inside the target — and said every
derived answer is that rule read in one direction or the other. So the split is by *direction*,
which makes the file layout the same statement the doc makes:

- `hierarchy.rs` — the containment everything else climbs, and the only thing `rollup` descends.
- `deps.rs` — the source side, and `lifted_deps`, the primitive blocking/ranking/cycles all read.
- `ready.rs` — that side turned into the three predicates.
- `demand.rs` — the rule reversed: who is waiting on this, and the ranking built on it.
- `rollup.rs` — the one answer that only descends.
- `mod.rs` — the struct, its construction, and nothing else.

`cycles.rs` was already a sibling module adding methods to the same `Graph`, so this extends a
pattern the module had rather than introducing one.

Two functions were decomposed:

- **`dependency_line` had two hand-written symmetric sweeps** (cognitive 24, cyclomatic 13). Up
  followed dependencies-plus-children, down followed dependents-plus-parent, and the pair were
  the same flood over different neighbour sets. A `Direction` now names which, `sweep` is the
  flood, and `neighbours` is the only place the two differ. The comment that mattered — that the
  sweeps must never cross, or the result stops being a line and becomes the connected component,
  cousins and all — now sits on the enum that enforces it.
- **`demand_edges` fed one map from two unrelated channels** (cognitive 17). Containment demand
  and lifted-dependency demand are now a function each, and `live_subtree` names the
  drop-the-terminal-issues filter both ends applied inline.

## Acceptance criteria
- [x] every `src/graph/mod.rs` entry gone from all six categories of `quality-report.json`
- [x] no threshold moved, and no other file made worse to pay for it
- [x] every derived answer over the real graph unchanged, proven not asserted
- [x] `cargo fmt`/`clippy`, `cargo test --all`, `conformance/run.py`, `scripts/tests` all pass

## Notes
Every category `graph/mod.rs` touched improved, and the deltas sum to exactly its former 146 —
so nothing was moved sideways into another file:

| category | before | after | delta |
|---|---|---|---|
| `file_lines` | 559 | 464 | −95 |
| `file_functions` | 201 | 176 | −25 |
| `function_cognitive` | 135 | 114 | −21 |
| `function_cyclomatic` | 246 | 241 | −5 |

`src/graph/` is now `mod.rs` plus `hierarchy`, `deps`, `ready`, `demand`, `rollup` and the
pre-existing `cycles` — seven files against a `module_files` threshold of 20. The one violation
left under `src/graph/` is `cycles.rs::find_cycles`, untouched and unchanged.

**How it was verified.** This module is the one whose output every other module reads, so a
golden had to cover the *answers*, not just the rendering. Two, both byte-for-byte identical:

- **The differential dump already in the file.** `dump_real_graph_answers_for_differential_comparison`
  is an opt-in harness (`TRCK_DUMP_GRAPH`) written for the Python-to-Rust cutover: it answers
  `leaf`/`blocked`/`ready`/`pct`/`lifted`/`cone size`/`demand source` for every issue in this
  repo's real tracker, plus the full ranking. 242 lines over 241 issues, unchanged.
- **The CLI surface those answers reach.** `deps` over the whole graph and every issue's line
  across the flag combinations, `list` in five forms, `ready`, `next`, `summary`, `check` —
  ~1250 invocations, 30,671 lines, unchanged.

Coverage was already good here, unlike `repo.rs`: 63 conformance fixtures (33 `deps`, 30
`ready`/`next`) plus 20 unit tests. The 20 moved to the module each exercises, and 8 were added
for guards nothing had been making earn their existence — `subtree` and `leaf_rollup` not hanging
on a parent cycle, the demand cone not hanging on a dependency cycle, a parent pointing at a
missing id ending the spine, `is_ready` on an id that is not in the index, demand travelling a
whole chain, and the two `dependency_line` properties the new `Direction` split makes explicit
(siblings stay cousins; each direction reaches only its own cone).
