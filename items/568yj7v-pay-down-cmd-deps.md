# pay down cmd_deps

## Summary
`cmd_deps` was the worst function left in the baseline, and the only one topping three
categories at once: 29 cyclomatic, 25 cognitive, 68 lines. It read as one function because
`deps` reads as one verb, but it was doing two unrelated things — deciding *which* ids belong
on screen, and *drawing* them — with the deciding half branching four ways and three of those
ways answering the verb outright without a graph ever being drawn.

The split is along that seam, into a new `src/query/deps.rs`. A `Deps` value holds what both
halves need (graph, focal issue, abbreviations, flags); `select` returns either the id set or
the finished text, which is what keeps the three degenerate answers from being early returns
threaded back through the drawing code.

Putting it in its own file is the same lesson `pay down dispatch and cmd_list` learned:
splitting a function adds functions to its file, and the ratchet does not let one category pay
for another. `query/mod.rs` is now just `ready`/`next` and the re-exports.

## Acceptance criteria
- [x] `cmd_deps` off the cognitive, cyclomatic and length violation lists entirely
- [x] no category worse: `ratchet compare` passes
- [x] behaviour unchanged — no conformance fixture regenerated

## Notes
Totals: cognitive 239 → 224, cyclomatic 376 → 357, function_lines 102 → 84, file_functions
294 → 292; `file_lines`, `function_args` and `module_files` unmoved. `cmd_deps` itself: 68
lines → 11, cyclomatic 29 → 8, cognitive 25 → 1.

The `--requires`/`--blocks`-without-an-id check moved into `select`, where the option
combination is actually decidable — that is what took `cmd_deps` under the cyclomatic
threshold rather than merely nearer it. Error precedence is unchanged: an unresolvable id
still wins, because the id is resolved before the graph is built.

Evidence that nothing needed changing to prove behaviour held: 192 unit tests and 242
conformance fixtures pass as they stand, none regenerated.
