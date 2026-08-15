# discovery: a narrowed fetch refspec hides the tracker, and setup-git widens it

## Summary
A default clone's refspec is `+refs/heads/*:refs/remotes/origin/*`, so `origin/trck-issues`
arrives with every ordinary `git fetch` and stays fresh for free. A clone made with
`--single-branch` or `--depth` — which is what `actions/checkout` does by default — narrows it to
`+refs/heads/main:refs/remotes/origin/main`. There, `origin/trck-issues` does not exist and never
will, however many times you fetch.

`resolve_tracker_source` then falls through to the walk-up's error: *no tracker found*. That is
honest about what it saw and wrong about what is true — there is a tracker, this clone simply
cannot see it, and no amount of the thing the message implies (make one) will help.

Choosing `refs/heads/trck-issues` over `refs/trck/issues` already fixed the *default* refspec case;
`source.rs` says so where it rejects the alternative. It did not fix the narrowed one.

This only becomes reachable at #8d22h6x: until `issues/` leaves `main`, the working-tree tracker
wins and a single-branch clone still finds it.

## Acceptance criteria
- [ ] A clone whose refspec does not cover the tracker branch gets a diagnostic naming the refspec
      and the remedy, distinct from "no tracker found".
- [ ] The two cases are told apart by what is actually true — the remote has a `trck-issues`
      branch — not by guessing from the refspec string.
- [ ] `trck repo setup-git` adds a refspec covering the tracker branch when the configured one does
      not, and is idempotent.
- [ ] A repository that genuinely has no tracker still reads the old wording.
- [ ] Offline, the diagnostic degrades to something true rather than to a network error.

## Notes
Supersedes #nuemwhc, which asked for a staleness warning on reads. That turned out to be a git
problem rather than a trck one: `origin/trck-issues` is exactly as stale as `origin/main`, refreshed
by the same `git pull` and warned about by neither. What is left after subtracting it is not age but
**absence**, which is this.

`repo setup-git` is already the per-clone configuration verb — it registers the merge drivers for
the same reason, that a clone cannot inherit them from the repository.

Checking the remote costs a network round trip, so it belongs on the error path only: the fast path
is a ref that resolves, and nothing asks the remote anything until discovery has already failed.
