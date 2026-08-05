# conformance: convert the changelog output tests

## Summary
`changelog [SINCE]` is the release-notes verb. Source: `test_changelog.py` — 18 tests, of which
the `parse_since` half is genuinely internal and stays in Python.

## Acceptance criteria
- [ ] Grouping and ordering of shipped issues.
- [ ] The `SINCE` cutoff: bare date, full timestamp, and what a malformed one does (error path
      and exit code).
- [ ] Resolution handling — superseded/wontfix/duplicate versus a plain ship.
- [ ] Parent/child rollup in the output, if any.
- [ ] Python originals for the converted cases deleted; `parse_since` unit tests stay.

## Notes
Part of #xm6h2qn.
