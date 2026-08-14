# writes: blobs to tree to commit-tree, then CAS the local ref

## Summary
Take #A3's changeset and turn it into a commit without a checkout: `hash-object -w` the
blobs, build the tree against a temp index (`GIT_INDEX_FILE`), `commit-tree` with the base
commit as parent, then `update-ref` with the old value as a compare-and-swap.

Base is `refs/heads/trck-issues` when it exists, else `refs/remotes/origin/trck-issues`. When
neither exists this is the first write: create the ref.

The local branch is not a convenience, it is the write-ahead log — it anchors the commit against
gc, which is what keeps a failed push from losing the issue just filed.

## Acceptance criteria
- [ ] A write verb produces one commit whose tree matches what the directory backend would have written, byte for byte.
- [ ] `refs/heads/trck-issues` advances under CAS; a concurrent local update loses the race cleanly rather than clobbering.
- [ ] The first write against a tracker with no ref creates it.
- [ ] An unset `user.email`/`user.name` produces a clear error naming the git config to set, not a raw `commit-tree` failure.
- [ ] The working tree and index of the current checkout are untouched — verified from a dirty tree on an unrelated branch.

## Notes
`GIT_INDEX_FILE` pointing at a temp file is what keeps the caller's index out of it; that is the whole reason this is not `git add`.
