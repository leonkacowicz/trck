# pay down the edit verbs

## Summary
`src/verbs/edit.rs` was the longest file left in `src/` (383 lines, 45 functions) and carried
four of the five worst functions by cyclomatic complexity plus the worst by length. The five
verbs in it shared only a shape — load, resolve the id, mutate, guard, `finalize` — so the
file became a directory with one file per verb, which is what paid for splitting the functions
inside them.

Two of the splits are worth more than the metric they moved:

- **`set` now checks, then applies.** It used to validate and assign in the same pass, safe
  only because nothing was persisted until it returned — a much weaker guarantee than not
  being able to fail. `check_scalar_edits` refuses in the order the edits are applied, then
  `apply_scalar_edits` is infallible, so a half-edited row is not a state the verb can reach.
- **`mv` takes an `MvOpts`.** It was the last five-argument function in the tree, and the two
  extra arguments were exactly the two facts the `start`/`review`/`done` aliases fill in. With
  the option-building moved to `cli/opts.rs` beside `new_opts`/`set_opts`, the four verbs
  collapse to one arm in `dispatch_mutating`, which was itself the worst cyclomatic offender.

## Acceptance criteria
- [x] no category worse: `ratchet compare` passes
- [x] `cmd_new`, `cmd_mv`, `cmd_dep` off every violation list
- [x] behaviour unchanged — no conformance fixture regenerated

## Notes
Totals: file_functions 292 → 272, file_lines 1108 → 1025, function_args 14 → 13, cognitive
224 → 207, cyclomatic 357 → 306, function_lines 84 → 57. Per function: `cmd_new` 74 lines /
23 cyclomatic → 23 / 8, `cmd_mv` 53 / 22 → 15 / 10, `cmd_set` 31 / 24 → 19 / 14,
`apply_scalar_edits` 45 / 18 → two functions at 12 and 11, `cmd_dep` 15 → 10,
`dispatch_mutating` 30 → 24.

Error precedence was the thing to be careful about: both `new` and `set` validate
left-to-right in the order the fields are given, so which complaint comes back does not depend
on which check happens to be cheap. `build_row` derives its values in sequence rather than
inside the struct literal for that reason.

192 unit tests and 242 conformance fixtures pass as they stand, none regenerated.
