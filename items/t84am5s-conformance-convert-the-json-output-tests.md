# conformance: convert the --json output tests

## Summary
`--json` on `list`/`show`/`deps`/`ready`/`next` — one JSON document per invocation. Source:
`test_json_output.py` (34 tests, 63 assertions), the largest single block left.

## Acceptance criteria
- [ ] One parseable document per verb; `show --json` folds the body in rather than appending a
      `--- body ---` trailer.
- [ ] Field names and nesting for each verb.
- [ ] Filters and flags reflected in the JSON the same way they are in the human view.
- [ ] Python originals deleted; assertion count carried over is checked.

## Notes
**Blocked on #gh363h3**, which implements `--json` in the Rust engine: fixtures written before it
would sit red on Rust with nothing to fix them. See also #agm4zhf — Rust currently *accepts*
`--json` and silently prints human output, which must stop lying first. Part of #xm6h2qn.
