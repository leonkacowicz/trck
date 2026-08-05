# conformance: convert the error-path and exit-code tests

## Summary
An engine that fails differently is as wrong as one that succeeds differently. Collect the error
paths scattered across the read/write suites and `test_cli.py` into fixtures asserting
`expected.err` and `expected.code` together.

## Acceptance criteria
- [x] Unknown id, ambiguous id prefix, and the candidate list an ambiguous prefix prints.
- [x] Unknown status/priority/kind/resolution values.
- [x] Missing required arguments and unrecognized flags (exit 2, not 1).
- [x] A malformed or inconsistent tracker: the diagnostic, not a stack trace.
- [x] Exit codes asserted explicitly everywhere, not left to the absent-means-0 default.
- [x] Python originals deleted; assertion count carried over is checked.

## Notes
31 fixtures; all pass on both engines. Ratchet 145 -> 176. **Retired:** 22 Python tests / 29
assertions, plus two dead helpers.

**Two tiers of assertion, on purpose.** Where both engines word a diagnostic identically the
fixture pins stderr *and* the code. Where the text is inherently implementation-specific it pins
the **exit code only** and stays silent on stderr — "absent means not asserted" exists for this.
That applies to argparse's usage block (Python) versus the Rust engine's terse `error:` line: the
*contract* is exit 2, which both honour, and pinning argparse's prose would make the port fail for
being written in Rust. Same for a JSON parser's positional detail inside `invalid JSON (...)`.

**`--kind` has no values to reject.** The criterion inherited it from #xm6h2qn, but the vocabulary
change removed the field — same finding as #rqs5ptd. Nothing to convert.

**Found one Rust bug and two diagnostic gaps.**
- *Behavioural, fixed:* `list --field status=backlog` was accepted by Rust and evaluated as a
  lookup of a custom field named `status`, which cannot exist — so the filter appeared to work
  while meaning something else. `check_field_key` now guards the read path as it did the write
  path.
- *Filed:* #gw9qnmw (duplicate-id error drops which ids and their statuses) and #d6mquku
  (re-parent cycle error drops which authored edge closes the loop). Both fixtures assert the exit
  code only until those close, and each fixture comment names its issue.

**Left in place:** internal validation unit tests (`Issue.from_dict` rejections, config parsing,
merge/update/init specifics). They are not CLI error paths — they never exec the binary — so a
fixture cannot express them.

#6vpkjxg (validation-error ordering) is still undecided, so it is not pinned here; the note in
that issue says which fixture should pin it once it is.
