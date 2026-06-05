# version: suppress no-tracker stderr noise when run outside a tracker

## Summary
`trck version` outside any tracker prints `error: no tracker found here` to stderr (from `die` inside `find_tracker`) before printing the version. `cmd_version` should resolve the tracker dir silently.

## Acceptance criteria
- [ ] `trck version` with no tracker prints only the version; stderr is clean

## Notes
