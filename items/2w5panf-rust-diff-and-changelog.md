# rust: diff and changelog

## Summary
`diff` compares tracker state between revisions over a VCS-agnostic source seam with git as a
layer on top; `changelog` reports what shipped since a date.

Half of `diff` is still unbuilt in Python — four of its six children are open. Worth deciding
whether to port what exists or build the remainder directly in Rust, rather than doing it twice.

## Acceptance criteria
- [ ] The source seam and change model, git layer included: revision specs and a HEAD default.
- [ ] Whichever layouts have landed at porting time.
- [ ] `changelog` since a date or timestamp.
- [ ] A recorded decision on porting versus finishing in Rust, and `u5fc5vm` updated to match.
