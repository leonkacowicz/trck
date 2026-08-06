# rust: the CLI help still says the port is in progress

## Summary
`trck --help` opens with:

> The port is in progress: the mutating verbs work, the read verbs do not yet.

Both halves are false — the read verbs pass every fixture, and 225/225 of the conformance suite
runs green against the binary. `crates/trck/src/main.rs` carries the same claim in its module
docs. It was true when it was written and nobody has had a reason to look at it since.

This is small, but it is the first thing a user reads, and it would be an embarrassing line to
ship in the first release. Hence it blocks `#7htpzdd` rather than the cutover as a whole.

## Acceptance criteria
- [ ] The help preamble describes what the binary is, not where the port had got to.
- [ ] `crates/trck/src/main.rs`'s module docs likewise.
- [ ] Nothing in the shipped text is a claim that goes stale on the next commit — the standing
      arrangement is that progress is measured by `conformance/`, not described in prose.
