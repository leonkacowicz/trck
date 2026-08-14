# reads: resolve index, bodies and trck.json from a git ref

## Summary
Implement the ref-backed source behind #A2's accessors: `git show <ref>:index.jsonl`,
`<ref>:items/<id>-<slug>.md`, `<ref>:trck.json`. No worktree, no checkout, no fetch.

The tracker is at the branch root, so `git_tracker_prefix` collapses to empty — the prefix logic
`diff.rs` needs for a tracker nested in a tree still applies to the working-tree case.

## Acceptance criteria
- [ ] All nine read verbs work from any branch, with a dirty tree and no `issues/` directory present.
- [ ] A body missing from the ref while the index lists it produces the same diagnostic a missing file does today.
- [ ] A ref that does not resolve, or resolves to a tree that is not a tracker, is a clear error naming the ref.
- [ ] No read verb invokes `fetch`.
- [ ] `trck path` says something honest for a ref-backed tracker rather than printing a path that does not exist.

## Notes
The prefix collapsing to empty is the only part of the read path that is genuinely new; the plumbing is `diff.rs`'s, moved in #A1.
