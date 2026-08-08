# pay down the gutter renderer

## Summary
`src/gutter/mod.rs` was the last file violating **every** category at once, and it held the
single largest excess anywhere in the baseline: 464 counted lines against a threshold of 300,
50 function spaces against 20, plus seven functions over the complexity limits and two over the
argument limit.

It read as one file because "draw the DAG" reads as one job. It is four, and they run in
sequence: build the edge set, reduce it, order the nodes, draw the cells. Each is now a module
named after its step, and `mod.rs` is the three entry points that string them together.

The split is what paid for splitting the functions inside it — the lesson `pay down dispatch and
cmd_list` and `pay down cmd_deps` both learned: more functions in the same file is what
splitting a function *means*, and the ratchet does not let one category pay for another.

Three of the splits are worth more than the metric they moved:

- **`shorten_lanes` became a `Search`.** The worst function left at 61 lines, 18 cyclomatic,
  17 cognitive. It was one function because the local search is one idea, but the cost model
  (weights, the precedence window) and the scan that walks it are different things, and the
  positions-and-prefix-sums state was being rebuilt inline at three separate points. `Search`
  owns that state with one `resync`, and `shorten_lanes` is now the loop that says *first
  improvement, until nothing helps*.
- **`draw_row` takes a `Transition`.** It was the last six-argument function in the tree, and
  five of the six were one thing: the lanes above and below a row, the node's column, and which
  columns close into it and open beneath it. They are only meaningful against each other, so
  `arriving`/`started` had no business travelling separately from the `top`/`bottom` they were
  computed from. Lane assignment became methods on that type, and drawing became `row.draw(n)`.
- **`glyph` became a table.** Twelve match arms mapping direction sets to box-drawing
  characters, which is a lookup table written as control flow — 13 cyclomatic for what is data.
  It moved into `canvas.rs`, where the cells it draws already live, and the keys are built from
  a `BTreeSet` so a new test asserts every key is in sorted order: one written out of order
  would be dead code that looks correct and never matches.

`filter_done`'s three booleans became a `DoneFilter`, which also gave the third one somewhere to
say what it means — a rooted view shows one issue's whole line, done or not, and only the
whole-graph view drops finished components.

## Acceptance criteria
- [x] every gutter entry gone from all six categories of `quality-report.json`
- [x] no threshold moved, and no other file made worse to pay for it
- [x] `deps` output byte-identical across every flag combination
- [x] `cargo fmt`/`clippy`, `cargo test --all`, `conformance/run.py`, `scripts/tests` all pass

## Notes
Every category improved, and each total dropped by exactly the gutter's former share — so
nothing was moved sideways into another file:

| category | before | after | delta |
|---|---|---|---|
| `file_lines` | 805 | 641 | −164 |
| `function_cognitive` | 207 | 152 | −55 |
| `file_functions` | 249 | 219 | −30 |
| `function_cyclomatic` | 306 | 279 | −27 |
| `function_lines` | 57 | 46 | −11 |
| `function_args` | 13 | 10 | −3 |

`src/gutter/` is now `mod.rs` plus `edges`, `reduce`, `components`, `order`, `shorten`, `rows`
and `canvas` — eight files against a `module_files` threshold of 20, which stays at zero.

**How it was verified.** A pure refactor of a renderer needs a golden, and the conformance
suite's gutter fixtures are small by design. So `deps` was run over this repo's own tracker —
234 issues, the whole graph and every issue's line, across every combination of
`--requires`/`--blocks`/`--full`/`--fanout`/`--omit-done`/`--include-done-chains`/`--json`, plus
`list`/`ready`/`next`/`check` as insurance that nothing else shifted: 273 invocations, 2714
lines, captured against a pristine build and diffed byte for byte. Identical.

Two refactors changed evaluation order and are worth naming, since the golden is what backs
them:

- `carried_above` is now asked once per ancestor rather than once per edge that ancestor
  authored. Its answer never depended on the target, so this is the same result and strictly
  less work.
- `transitive_reduction`'s `implied` check reads a sibling edge's reachability; the recursion
  guard that makes `edge_reach` terminate on a malformed cycle is unchanged, and the test
  covering it moved with it.

Four tests were added alongside the six that moved: the glyph table's key ordering, transitive
reachability down a chain, and two fixed points of the lane search (a chain is already minimal;
an edgeless order is untouched).
