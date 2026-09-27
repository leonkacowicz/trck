## Summary

A `discovery: ref` fixture runs `cmd` from `repo/nested`, below the repository root — but
`setup` writes to a plain directory before the ref is built, and the runner refuses artifact
goldens (`expected.index.jsonl`, …) in ref mode because there is no working-tree path to read
them from. So a write verb under test in ref mode is asserted on its stdout and exit status
only, and a write that deleted most of the branch still passed. That is how #pa9jtd5 — every
write from a subdirectory dropping the tracker tree — got past a suite that already ran from a
subdirectory.

The integration tests in `tests/ref_writes.rs` cover the specific case now, but the spec
itself cannot state it.

## Acceptance criteria

- [ ] A ref-mode fixture can assert the branch's files after `cmd` — e.g. read the artifact
      goldens out of the ref with `git show <ref>:<path>`, and/or a golden listing its tree.
- [ ] A fixture asserting a write from `nested` keeps the whole tree.
