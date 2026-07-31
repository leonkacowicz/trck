# tests: test_an_id_prefix_resolves flakes when a 2-char id prefix collides

## Summary
`TestScopedReady.test_an_id_prefix_resolves` builds an epic plus children with real (random)
ids and then resolves the epic by its **first two characters**. Ids are random 7-char base32,
so two of the fixture's ids can share a 2-char prefix; `resolve_ref` then correctly reports
`ambiguous id prefix` and the test dies with `SystemExit: 1`. Roughly one run in a few dozen.

Observed while cutting v0.24.0 — the suite failed once with this error and passed on re-run,
five consecutive single-test runs green.

## Acceptance criteria
- [ ] The test no longer depends on a random prefix being unique
- [ ] The suite passes deterministically across repeated runs

## Notes
`tests/test_read.py:871`. The fix is to derive the prefix from the actual ids rather than
hard-coding a length — take the shortest prefix that is unique among the fixture's ids (the
same rule the CLI's shortest-unique-prefix display uses), or seed the fixture with fixed ids.
