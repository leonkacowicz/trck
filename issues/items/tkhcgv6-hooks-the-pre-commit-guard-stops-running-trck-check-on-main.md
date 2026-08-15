# hooks: the pre-commit guard stops running trck check on main

## Summary
`scripts/hooks/pre-commit` runs `trck check` before every commit. Once the tracker is on a
ref, a commit on `main` cannot affect it, so the check is either a no-op or — worse — an error on
a tracker that is not in the tree.

Drop it there. The write path's own validation (#C9's changeset, built from validated rows) is
the real gate, and it runs on the only commits that can break the tracker.

## Acceptance criteria
- [ ] The pre-commit hook no longer runs `trck check` on a code commit.
- [ ] `scripts/tests` covering the hook are updated, and the hook still does whatever else it does.
- [ ] `CONTRIBUTING.md`/`CLAUDE.md`'s `git config core.hooksPath scripts/hooks` instruction is still accurate.

## Notes
Small, but it has to be in before the flip. Not the first commit *after* the flip — the flip
commit itself: `git rm -r issues/` is a tracker change, so the hook runs, and it runs
`trck check` against a directory that commit is removing.

Rehearsed in another repo (2026-08-15) rather than reasoned about: `git rm -rq issues && git
commit` aborted with `trck inconsistent — aborting commit`. An earlier dependency edit had this
issue depending on the flip; that was wrong, and it is back the other way round.
