# config: in-review status, actionable flag, review alias

## Summary
Extend `DEFAULT_CONFIG` to `backlog → ongoing → in-review → done` and add the
`"review": "in-review"` alias. `in-review` carries **no role** — the one-each
`initial`/`active`/`terminal` constraint the rollup reasons about is untouched.

Introduce the generic per-status flag `"actionable": false` (default `true`), meaning
"an issue in this status is not available to pick up", plus an `is_actionable(cfg, name)`
helper. This keeps the engine ignorant of the word "review": any project can model its
own waiting states (`qa`, `awaiting-deploy`, …) with no engine change.

## Acceptance criteria
- [ ] `DEFAULT_CONFIG` has `in-review` (no role) and the `review` alias
- [ ] `check_status_roles` still passes on the new default
- [ ] `is_actionable` defaults to `True`, returns `False` when opted out
- [ ] `validate` errors when `actionable` is present but not a boolean

## Notes
Blocks the `ready`/`next` change, the `review` verb, and this repo's own adoption.
