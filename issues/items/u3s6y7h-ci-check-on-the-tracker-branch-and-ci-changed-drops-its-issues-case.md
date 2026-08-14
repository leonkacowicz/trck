# CI: check on the tracker branch, and ci_changed drops its issues/ case

## Summary
`trck check` moves to its own workflow gated on `push: branches: [trck-issues]`. Code CI
stops knowing the tracker exists, and `scripts/ci_changed.py` loses its `issues/` case entirely —
after the flip a code PR structurally cannot contain a tracker change.

Per the allowlist rule in `CLAUDE.md`: update `scripts/tests/test_ci_changed.py` **first**. A
classifier that wrongly says 'skippable' does not turn a check red, it makes the checks green by
never running them.

## Acceptance criteria
- [ ] A push to `trck-issues` runs `trck check` against the branch and fails on a bad tracker.
- [ ] `scripts/ci_changed.py` no longer special-cases `issues/`, with the test updated in the same change and written first.
- [ ] The code workflows no longer reference `issues/` in any path filter or gate.
- [ ] The matrix job's bare-name reporting is unaffected — it still always runs.
- [ ] Branch protection on `main` still lists the same required checks.

## Notes
Must land with or before the flip: after `issues/` leaves `main`, a `trck check` step that looks in the checkout has nothing to check.
