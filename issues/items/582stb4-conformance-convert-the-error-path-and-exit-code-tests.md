# conformance: convert the error-path and exit-code tests

## Summary
An engine that fails differently is as wrong as one that succeeds differently. Collect the error
paths scattered across the read/write suites and `test_cli.py` into fixtures asserting
`expected.err` and `expected.code` together.

## Acceptance criteria
- [ ] Unknown id, ambiguous id prefix, and the candidate list an ambiguous prefix prints.
- [ ] Unknown status/priority/kind/resolution values.
- [ ] Missing required arguments and unrecognized flags (exit 2, not 1).
- [ ] A malformed or inconsistent tracker: the diagnostic, not a stack trace.
- [ ] Exit codes asserted explicitly everywhere, not left to the absent-means-0 default.
- [ ] Python originals deleted; assertion count carried over is checked.

## Notes
`#6vpkjxg` (validation-error ordering between the engines) should be pinned by a fixture here
once it is decided. Part of #xm6h2qn.
