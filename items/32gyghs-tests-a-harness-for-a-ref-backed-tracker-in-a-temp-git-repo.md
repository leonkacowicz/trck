# tests: a harness for a ref-backed tracker in a temp git repo

## Summary
The ref layer cannot be specified in `conformance/`: fixtures exec the binary against a
plain directory in a temp dir with no git anywhere, and that method is the reason a hosted
backend was ruled out. It stays as it is.

So the ref layer needs its own integration harness in `tests/`: build a temp repo, create an
orphan branch whose root is a tracker, run the binary against it, assert on stdout. Model it on
`tests/broken_pipe.rs` and the two-writer test from #ey2aruc, which already stand up real
scenarios around the binary.

## Acceptance criteria
- [ ] A helper builds a temp repo with an orphan tracker branch, a second clone of it, and a dirty working tree on an unrelated branch.
- [ ] Tests skip cleanly, not fail, when git is absent — the way `tests/app_js.rs` skips without node.
- [ ] Temp repos are removed even when an assertion fails.
- [ ] `conformance/run.py` is not modified.

## Notes
Every task in the reads and writes tranches asserts through this harness.
