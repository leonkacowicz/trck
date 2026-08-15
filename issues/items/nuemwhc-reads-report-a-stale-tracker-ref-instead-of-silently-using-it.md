# reads: report a stale tracker ref instead of silently using it

## Summary
Reads deliberately never fetch — too slow, and it would put the network on the path of
every `trck list`. But `trck next` planning against a week-old `origin/trck-issues` is the
time-travel bug this epic exists to kill, wearing a different hat.

So surface the ref's age when it is beyond a threshold, rather than letting staleness be
silent.

## Acceptance criteria
- [ ] A ref older than the threshold produces a warning on stderr naming `trck sync`; stdout is unchanged, so scripts and `--json` consumers are unaffected.
- [ ] The threshold and its presentation are settled and written down in the body before the task closes.
- [ ] The warning is suppressible, and is not emitted when the local ref is ahead (you are the one who wrote it).
- [ ] No read verb fetches.

## Notes
Open question carried from the epic: what the threshold is, and whether it belongs on every read or only on the planning verbs (`next`, `ready`).

## Resolution — superseded by #yqb2afy
This is a git problem wearing a trck hat, and the threshold question has no good answer because the
premise does not hold.

**Staleness is not detectable locally.** A clone can measure how long since it last reached the
remote; it cannot measure how far behind it is. So the only warning it could honestly emit is about
its own ignorance, not about the tracker.

**And that ignorance is already bounded by ordinary git use.** The default refspec is
`+refs/heads/*:refs/remotes/origin/*`, and `trck-issues` is a `refs/heads/` branch — so every
`git fetch` or `git pull` anyone runs in the repo refreshes `origin/trck-issues` for free.
`origin/trck-issues` is exactly as stale as `origin/main`, refreshed by the same command, and git
warns about neither. A warning here would fire constantly on the steady state and be filtered out
long before the one time it mattered.

Two smaller conclusions worth keeping, both explored and dropped:

- **A TTL-gated auto-fetch on reads** (fetch at most once per interval, otherwise use local) rate-limits
  the network but does not bound the worst case: a read that blocks is a read that can hang behind a
  VPN or an ssh prompt. Detaching it fixes that — but on a default clone it duplicates work `git pull`
  already did, and `#eemua4s` covers the case where a live process wants it on a timer.
- **The staleness clock, had one been built, is last-successful-fetch, not tip age.** Tip age
  conflates a quiet tracker with an unchecked one, so a repo where nobody filed anything for a week
  would nag forever.

What is left after subtracting all of that is not age but **absence**: a clone whose refspec was
narrowed never receives the branch at all, and the diagnostic it gets today says the tracker does not
exist. That is #yqb2afy.
