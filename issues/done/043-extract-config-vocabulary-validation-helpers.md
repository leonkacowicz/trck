# Extract config-vocabulary validation helpers

## Summary
"Is this value in the configured vocabulary?" checks for priority, kind, points, and
resolution are re-implemented across the command handlers and `validate`, each with its
own `die(...)` wording:

- priority — `cmd_new` (`trck:1000-1001`), `cmd_set` (`trck:1054-1056`)
- kind — `cmd_new` (`trck:1006-1008`), `cmd_set` (`trck:1074-1077`)
- points (non-negative) — `cmd_new` (`trck:1004-1005`), `cmd_set` (`trck:1058-1063`)
- resolution — `cmd_mv` (`trck:1037-1040`)
- and the read-back equivalents in `validate` (`trck:586-603`)

Because the create-time and validate-time rules live in separate copies, they can
drift (e.g. a future rule added in `validate` but missed in `cmd_set`). Extracting a
few small helpers (e.g. `check_priority(cfg, v)`, `check_kind(cfg, v)`,
`check_points(v, is_leaf)`, `check_resolution(cfg, v)`) gives one definition per rule.

## Acceptance criteria
- [ ] Vocabulary/value checks for priority, kind, points, and resolution each have a
      single shared implementation used by the command handlers.
- [ ] Error messages remain clear and still list the configured options (current
      `die` messages include the allowed set — preserve that).
- [ ] `validate` and the create/edit paths agree by construction (share the predicate
      where it makes sense, even if `validate` accumulates errors rather than dying).
- [ ] Tests for the existing rejection messages still pass.

## Notes
- The command paths `die()` on the first bad value; `validate` *accumulates* into an
  error list. A shared helper can return a bool/optional-message that each caller turns
  into either a `die` or an `errors.append` — keep that split.
- Don't over-abstract: a handful of named one-liners is the goal, not a framework.
