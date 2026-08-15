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
- [x] A write verb produces one commit whose tree matches what the directory backend would have written, byte for byte.
- [x] `refs/heads/trck-issues` advances under CAS; a concurrent local update loses the race cleanly rather than clobbering.
- [x] The first write against a tracker with no ref creates it.
- [x] An unset `user.email`/`user.name` produces a clear error naming the git config to set, not a raw `commit-tree` failure.
- [x] The working tree and index of the current checkout are untouched — verified from a dirty tree on an unrelated branch.

## Notes
`GIT_INDEX_FILE` pointing at a temp file is what keeps the caller's index out of it; that is the whole reason this is not `git add`.

Landed in PR #39. `src/verbs/backend/git.rs` holds `RefBackend`; `verbs::commit` picks between it
and `DirBackend` on `Ctx::source`, and no verb learns which it was operating on.

**A tree is built from an empty index**, so it holds exactly what it is given — everything the base
commit held has to be carried forward explicitly. That is `plan()`, kept a function of values so a
rename carrying its blob, a delete, and a rename-then-write all test without a repository. A rename
of a path the base lacks drops out rather than failing: refusing to build a tree over a body a
hand-edit already moved would strand the tracker rather than repair it.

**The parent and the CAS expectation are different questions.** The parent is what the tracker reads
as now — in a fresh clone, the remote-tracking ref. The expectation is what the *local* branch holds,
which there is nothing. So the first write in a clone creates the branch at a commit descending from
the remote's tip. Writes always land on `refs/heads/`; moving a remote-tracking ref locally would
make the clone lie about the branch it is named after.

`new` and `mv` stopped asking the filesystem, and report a body the way the tracker spells it —
a path for a directory, `<rev>:items/<id>-<slug>.md` for a ref. Visible output changed for
ref-backed trackers only.

CAS coverage is split deliberately: `update_ref` refusing a stale expectation is unit-tested in
`src/git/write.rs`; what the integration suite can hold a single process to is that each write
*re-reads* the ref, so a commit that landed in between becomes the next parent. A genuinely
interleaved race needs two processes and is not asserted here.

Held back on purpose: the commit subject is the op's own rendering — the subject convention and the
`Trck-Op` trailer are #93zhqbd — and pushing the anchored commit is #5w9d7sq. For #93zhqbd: `new`'s
op does not record the body, so an op replayed alone would produce an issue with none; the bytes are
in the changeset, so nothing is lost today.

Found and filed along the way: #jvk5637.
