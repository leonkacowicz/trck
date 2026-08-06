# pay down draw_row and validate

## Summary
The two worst cognitive scores left. Both were long for the same reason — a sequence of
things happening — and both had a seam nobody had needed to look for yet.

`validate` is a run of independent passes: dangling references, cycles, rollup agreement,
finished work waiting on unfinished. Each is now a function named after what it checks, and
`validate` is the list of them.

`draw_row` had two loops, one for edges arriving from above and one for edges starting below,
written out separately. They are the same drawing: the lane points at the node, the node
points back, and the cells between become a horizontal bridge. Only the vertical stroke and
which row the lane is read from differ, so those became the arguments and the two loops
became one call each. The canvas state moved into a type, which also let `pos` and `width`
stop being arguments to every method — they are fixed for the row.

The ratchet refused the first attempt twice, and was right both times: more functions in the
same file is what splitting a function means. `gutter.rs` and `validate.rs` became module
directories, and the arity regression from passing `pos` everywhere went away by making it
state.

## Acceptance criteria
- [x] `draw_row`: cognitive 28 over → 0, cyclomatic 13 → 0, length 26 → 0.
- [x] `validate`: cognitive 22 over → 0, cyclomatic 16 → 0, length 20 → 0.
- [x] Every category improved: cognitive 288 → 239, cyclomatic 411 → 382, function_lines
      148 → 102, file_lines 1259 → 1197, file_functions 331 → 330, function_args 15 → 14.
- [x] Behaviour identical — 182 tests and 228 fixtures pass untouched, including the 35
      gutter fixtures, which are ASCII art and would fail on a single misplaced glyph.
