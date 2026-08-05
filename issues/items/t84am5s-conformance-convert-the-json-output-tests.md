# conformance: convert the --json output tests

## Summary
`--json` on `list`/`show`/`deps`/`ready`/`next` — one JSON document per invocation. Source:
`test_json_output.py` (34 tests, 63 assertions), the largest single block left.

## Acceptance criteria
- [x] One parseable document per verb; `show --json` folds the body in rather than appending a
      `--- body ---` trailer.
- [x] Field names and nesting for each verb.
- [x] Filters and flags reflected in the JSON the same way they are in the human view.
- [x] Python originals deleted; assertion count carried over is checked.

## Notes
25 fixtures; both engines agree. Ratchet 197 -> 222. **Retired:** 35 Python tests / 66
assertions, four classes left with no tests, and the orphaned helpers and imports.

**Found a real gap in #gh363h3.** `ready --json` was missing `demand_priority` and
`demand_source` on a lifted row. The implementation had been checked by diffing both engines,
and it passed — because the graph used happened to contain no lifted row, so the outputs matched
for the wrong reason. Converting the tests is what caught it. Worth remembering: a hand-built
comparison scenario proves less than it appears to unless it exercises the branch.

**Kept in Python:** two `emit_json` unit tests. A fixture only ever sees a single invocation's
stdout, so it cannot distinguish "exactly one document" from "the first of two" — which is
precisely the regression `show --json` once had. That property has to be checked from inside.

Everything else in the file went, including the three `the_human_output_is_untouched` guards
(every human-view fixture pins its exact output, so a leak would fail there) and the two
`ids_are_never_abbreviated` guards (every golden shows full ids where an abbreviation would be
visible).
