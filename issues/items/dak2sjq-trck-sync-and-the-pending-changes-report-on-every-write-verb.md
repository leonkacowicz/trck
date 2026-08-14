# trck sync, and the pending-changes report on every write verb

## Summary
A failed push must be visible, not silent: the local ref is ahead, the work is safe, and
the verb says so.

```
#abc1234 done  (2 unpushed changes — run `trck sync`)
```

`trck sync` flushes pending commits and reconciles: it is also the natural home for the fetch and
fast-forward that reads deliberately do not do.

## Acceptance criteria
- [ ] Every write verb appends the pending count and the remedy when the push did not land.
- [ ] `trck sync` pushes pending commits, fetches, and fast-forwards the local ref.
- [ ] `trck sync` with nothing pending and nothing new is a no-op that says so and exits 0.
- [ ] `trck sync` offline reports the network failure and leaves pending work intact.
- [ ] The pending suffix goes to stdout only where it does not corrupt machine-readable output (`--json`, `path`).

## Notes
This is the verb the offline story rests on; without it a failed push is a silent data-loss story even though the commit is safely anchored.
