# diff: resolve revisions on the tracker branch

## Summary
`trck diff HEAD~5..HEAD` resolves revisions in the current checkout
(`src/diff.rs:230-260`). Once the tracker is on its own branch, `HEAD~5` on `main` is five
commits of engine code and means nothing to the tracker.

Revisions have to resolve against the tracker ref, with the prefix empty. Overlaps #wtmfdhr —
check what that one already assumes before starting.

## Acceptance criteria
- [ ] A revision spec resolves on the tracker branch for a ref-backed tracker, and in the checkout for a directory-backed one.
- [ ] `--from FILE`/`--from -` are unaffected.
- [ ] A revision that does not exist on the tracker ref is a clear error, not an empty diff.
- [ ] Pre-migration revisions still resolve after the subtree split (#E18).

## Notes
Must land before the flip: `diff` is the one read verb whose meaning changes rather than just its source.
