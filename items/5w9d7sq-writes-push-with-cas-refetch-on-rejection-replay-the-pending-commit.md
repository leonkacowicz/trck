# writes: push with CAS, refetch on rejection, replay the pending commit

## Summary
Push `<sha>:refs/heads/trck-issues` against the known remote tip. **No fetch before the
write:** a commit whose parent is not the current remote tip cannot be pushed, so either the base
was current (the validation ran against current data) or the push is rejected and it re-runs.
Fetch-on-rejection is identical in correctness to fetch-always at half the round trips.

On rejection: fetch, replay the pending operation from its trailer against the new tree, commit,
push again. Bounded retries, then report. Never force — a rejection means someone else's work
landed.

## Acceptance criteria
- [ ] The uncontended path is one network round trip.
- [ ] No write verb fetches before pushing.
- [ ] A contended push converges: two writers against one remote, both commits present afterwards, neither overwritten.
- [ ] Retries are bounded and the give-up path reports what is pending and what to run.
- [ ] `--force` never appears in a push invocation, under any path.

## Notes
Test shape follows #ey2aruc's contended-rebase test: two writers, one clone.
