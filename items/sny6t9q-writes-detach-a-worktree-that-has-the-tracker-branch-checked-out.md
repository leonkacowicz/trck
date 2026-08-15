## Summary

Nothing stops someone from checking `trck-issues` out — `git worktree add ../tracker trck-issues`,
or a plain `git switch trck-issues` in a second clone — and git will not defend that checkout from
us. `update-ref` is plumbing: the `die_if_checked_out` guard lives only in porcelain (`branch -f`,
`checkout`/`switch`, `worktree add`, and `fetch` when it writes a branch ref directly), so the CAS in
`git::refs::update_ref` moves `refs/heads/trck-issues` under a live checkout, silently and
successfully.

What that leaves is worse than a refusal. The worktree's `HEAD` jumps to the new commit while its
index and working tree stay at the old one, so `git status` there shows our commit **inverted** — the
issue body we just filed reads as a staged deletion, `index.jsonl` as reverted to its previous
contents. From that state a `git commit -a` produces a commit that undoes the tracker write and
pushes as a clean fast-forward: no conflict, no rejection, the row is gone.

The read path can trigger it too. `discovery::standing::reconcile` fast-forwards the local branch when
it is behind (`src/discovery/standing.rs`), so a plain `trck list` can yank `HEAD` out from under a
checkout that was minding its own business.

The fix is to make the other worktree honest about what it holds rather than to refuse the write.
Before moving the ref, detach any worktree sitting on it:

    git -C <worktree> update-ref --no-deref HEAD <sha it currently holds>

`--no-deref` overwrites the symref itself instead of what it points at, so `HEAD` becomes the literal
sha. Index and working tree are never read or written, a dirty tree is fine, and the per-worktree HEAD
reflog gets an entry, so `git checkout -` re-attaches. The checkout ends up faithful to the commit it
was actually on and no longer coupled to a branch that has moved on — which is the state the operator
believed they were in all along.

Writes detach; reads must not. When the ref is checked out somewhere, `reconcile` skips the
fast-forward and reads `origin/trck-issues` for that invocation instead — same content, zero mutation.
A `trck list` has no business touching anyone's `HEAD`.

Three cases decide whether the detach is safe:

- **Sequencer state.** A worktree mid-rebase, merge, cherry-pick or bisect has `rebase-merge/`,
  `MERGE_HEAD`, `CHERRY_PICK_HEAD` or `BISECT_LOG` in its git dir, and detaching under one corrupts the
  in-flight operation. Refuse the write and name the path; the operator finishes what they started.
- **Prunable or locked worktrees.** A missing directory cannot commit, so skip it silently. A locked
  one cannot be detached safely, so fall back to a warning and move the ref anyway.
- **Ordering.** The detach has to precede the CAS, because git will not refuse the move and there is
  nothing to learn from a failure. So a lost CAS leaves a gratuitous detach behind — harmless, one
  `git checkout -` from undone, and it belongs in the line we print.

The cost worth stating plainly: this is trck reaching into a checkout it does not own, which is the
property `#jgf9ktx` was sold on. It is a narrow violation — the `HEAD` symref only, never content, never
the index — and the alternative is a silent desync that reverts a tracker write through an ordinary
`git commit -a`.

## Acceptance criteria

- [ ] A helper reports which worktrees hold a given ref, from `git worktree list --porcelain`,
      distinguishing locked and prunable entries.
- [ ] Every ref-moving write detaches each such worktree with `update-ref --no-deref HEAD <sha>` before
      the CAS, and prints one line per detach naming the path and the sha.
- [ ] A detached worktree keeps its index and working tree byte-for-byte, dirty or clean, and
      `git checkout -` re-attaches it to the branch.
- [ ] A worktree in a sequencer state refuses the write, with a diagnostic naming the path and the
      operation in progress.
- [ ] A prunable worktree is skipped silently; a locked one warns and the ref still moves.
- [ ] `reconcile` does not fast-forward the local ref while it is checked out — it reads
      `origin/trck-issues` for that invocation instead, mutating nothing.
- [ ] Tests in `tests/` over `common::Scenario`, which already builds real repositories; conformance
      never runs `git init`, so this stays out of it.

## Notes

Found by reasoning about the branch layout, not by hitting it — the tracker still lives in `issues/`
here until `#8d22h6x`. It is independent of the migration tranche, though: the ref backend is already
the code path, so the hazard is live for anyone pointing `--ref` at a branch they also have checked
out.
