# cli: --pr on new/set/mv

## Summary
Three entry points for the PR link:
- `new --pr URL` — create with it already known.
- `set --pr URL|none` — edit or clear later, mirroring `--spec`.
- `mv --pr URL` — record the link *as part of* a move, the same shape as
  `done --resolution`. Unrestricted by status: linking a PR while `ongoing` is fine.

Every entry point validates through `check_pr`.

## Acceptance criteria
- [ ] `new --pr`, `set --pr`, `mv --pr` all store the value
- [ ] `set --pr none` clears it
- [ ] A non-URL value is rejected at each entry point with a clear message
- [ ] Each stays a single `finalize`, leaving the tracker `check`-clean

## Notes
`mv --pr` is what lets the `review` verb be one move with one line of output.
