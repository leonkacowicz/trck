# rust: check, validate and the repo maintenance verbs

## Summary
`check` is the contract enforcer — the pre-commit hook runs it — plus the rare-but-essential
`repo` verbs, including the merge drivers git itself invokes.

## Acceptance criteria
- [ ] `check`: every current validation, same messages, nonzero exit on error.
- [ ] `repo normalize`, `renumber`, `install-hook`, `setup-git`.
- [ ] `repo merge-index` and `merge-summary`. These run inside git during a merge, so their
      behaviour under conflict is part of the contract, and the row-wise merge must key on id
      exactly as today.
- [ ] The migration verbs, including whatever `rbast9r` lands.
- [ ] Refusal paths preserved: an unmigrated tracker is refused by every verb, naming the fix.
