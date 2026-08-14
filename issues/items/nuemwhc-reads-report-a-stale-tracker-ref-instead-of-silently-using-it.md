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
