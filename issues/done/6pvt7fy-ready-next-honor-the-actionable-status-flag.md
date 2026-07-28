# ready/next: honor the actionable status flag

## Summary
`Graph.is_ready` becomes `not terminal and actionable and leaf and not blocked`, so an
issue awaiting review is never proposed as the next thing to work on.

Blocking and rollup stay deliberately unchanged: `in-review` is non-terminal, so a
dependency on it still blocks (the PR isn't merged), and a parent whose children are
in-review still rolls up to the `active` status.

## Acceptance criteria
- [ ] An `in-review` leaf is absent from `ready` and never returned by `next`
- [ ] Moving it back to `ongoing` makes it ready again
- [ ] An issue depending on an `in-review` issue is still blocked
- [ ] A parent whose only child is `in-review` rolls up to `ongoing`

## Notes
`ready <id>` subtree scoping runs through the same predicate, so it follows for free.
