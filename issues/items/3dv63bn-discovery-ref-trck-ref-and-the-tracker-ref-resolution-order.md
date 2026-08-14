# discovery: --ref/$TRCK_REF and the tracker-ref resolution order

## Summary
Add the last step to `resolve_tracker_dir` (`src/discovery.rs:70`): when nothing else
resolves, ask git whether the tracker ref exists. Full order is `--dir` -> `$TRCK_DIR` ->
`--ref`/`$TRCK_REF` -> working-tree walk-up -> the tracker ref.

**A working-tree tracker beats the ref.** That keeps `trck init` unsurprising and it is what
makes the whole epic land in stages: nothing changes behaviour until `issues/` actually leaves
`main`.

`trck-issues` is convention, discovered by `git rev-parse --verify --quiet` — no marker file on
`main`, no new `trck.json` key, no new `check` warning.

## Acceptance criteria
- [ ] Resolution follows the documented order, and a working-tree tracker wins over an existing ref.
- [ ] `--ref REF` and `$TRCK_REF` override the conventional name; `--ref` beats the env var.
- [ ] An explicit `--ref` that does not resolve is an error naming the ref, not a fallback to the walk-up — same rule `--dir` already follows.
- [ ] Outside a git repo with no working-tree tracker, the 'no tracker found' diagnostic is unchanged.

## Notes
Not `refs/trck/issues`: outside `refs/heads/` it is not in the default fetch refspec, so a fresh clone reads as an empty tracker rather than an error. See the epic's rejected alternatives.
