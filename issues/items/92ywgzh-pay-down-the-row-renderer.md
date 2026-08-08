# pay down the row renderer

## Summary
`src/render/mod.rs` was a different shape of debt from the five paydowns before it: 63 excess
across three categories with **no `file_lines` violation at all**. At 284 lines it was not too
big — it was too tangled. Splitting it was necessary but not sufficient, and would on its own
have made `file_functions` *worse*, because decomposing three fat functions adds functions.

Four modules now, and the read verbs still import only the parent:

- `colour.rs` — which colour anything is, and whether there is colour (unchanged).
- `fields.rs` — reading one field off an issue as text.
- `annotate.rs` — the note at the end of a row.
- `rows.rs` — the row itself.
- `mod.rs` — the two pieces of id presentation everything else uses, and `python_list`.

Three functions came apart, each along a seam that was already there:

- **`field_value` (cyclomatic 27) was a 16-arm match over heterogeneous fields.** It is really
  three lookups: the five required strings, the seven `Option` strings, and the four that are
  not text at all — a count, two lists and a flag. Separating them also isolated the one real
  subtlety, which is that "absent" means different things to the two public readers: an empty
  string *is* a value `--field note=` set, but a column reading `note=` on every row carries no
  information. That difference is now a single `keep_empty` argument in one place instead of two
  near-identical match arms in two functions.
- **`block_annotations` (cognitive 27) was two independent notes in one function.** `needs_note`
  and `blocks_note` now answer separately, and the `(via #author)` decision moved to
  `author_suffix` — where it belongs, because it does not depend on the target and was being
  re-asked for every edge an ancestor wrote.
- **`render_rows` (cognitive 15) was a loop doing five things.** `Widths` names why the columns
  are measured over the batch, and `row_tags` / `annotation` / `field_suffix` are the pieces a
  row is assembled from.

## Acceptance criteria
- [x] every `src/render/mod.rs` entry gone from all six categories of `quality-report.json`
- [x] no threshold moved, and no other file made worse to pay for it
- [x] output byte-identical in **both** colour modes, not just the uncoloured one
- [x] `cargo fmt`/`clippy`, `cargo test --all`, `conformance/run.py`, `scripts/tests` all pass

## Notes
The three categories the file touched improved, and the deltas sum to exactly its former 63:

| category | before | after | delta |
|---|---|---|---|
| `function_cyclomatic` | 198 | 168 | −30 |
| `function_cognitive` | 108 | 86 | −22 |
| `file_functions` | 147 | 136 | −11 |

No threshold moved, and no entry anywhere in the report is new or worse.

**What remains under `src/render/` is `colour.rs::ansi` (cyclomatic 16, +6)** — untouched,
pre-existing, and in a different file from the one this issue is about. It is the obvious next
thing here if the module is revisited.

**Colour is half the output, so the golden had to cover it.** `paint` and the per-status and
per-priority code tables are exactly what this module decides, and the conformance suite runs
with `NO_COLOR=1` — so a suite-only check would have exercised one branch of every colour
decision and none of the other. The capture runs everything twice, once with `NO_COLOR=1` and
once with `FORCE_COLOR=1`: `list` in ten filter and sort forms, six `--show-field` combinations
covering every canonical key plus an unknown one, `ready`, `next`, `tree`, `deps`, `summary`, and
`show`/`list`/`deps` for all 269 issues. 1,656 invocations, 37,511 lines, 6,407 of them carrying
escape sequences. Byte for byte identical.

The first version of that harness quietly skipped the per-issue half in colour mode: it derived
the id list from `list --flat`, and with `FORCE_COLOR=1` the ANSI escapes broke the `grep`. The
symptom was a suspiciously small capture, which is the only reason it was noticed — worth
remembering that a golden that silently covers less than it claims is worse than none.

**Coverage.** 3 unit tests became 33. `render/mod.rs` had only ever tested `unique_prefix_lens`;
the rest of the module was covered solely through the conformance suite, which cannot say *which*
function was wrong. The additions are per-module: every canonical key being reachable through
`field_value` (so a key that fell through the three lookups could not silently read as a custom
field and answer `None` forever), the empty-value difference between the two readers in both
directions, each half of the blocking note including the cases where it stays quiet, column
alignment measured in characters rather than bytes, and the parent tag yielding to the tree
connector.

`test_graph`'s spec DSL grew `+label`, which it needed for the tag tests — hand-building an
`Issue` in the test would have been a second copy of the builder, which that module exists to
prevent. Adding it pushed `test_graph::issue` two over the cyclomatic limit, so the sigil parsing
moved into `attrs` beside it.
