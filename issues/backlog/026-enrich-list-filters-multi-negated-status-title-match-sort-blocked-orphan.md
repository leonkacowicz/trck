# Enrich 'list' filters: multi/negated status, title match, sort, --blocked, --orphan

## Summary
`list` today filters by a single value per field. Day-to-day triage wants a bit
more expressiveness without turning into a query language. Add a small, composable
set of options:

- multiple / negated status (e.g. `--status ongoing,backlog`, `--status !done`).
- `--match <text>` substring filter on the title.
- `--sort <field>` (priority, points, id, created) to order output.
- `--blocked` — only issues with at least one non-terminal dependency.
- `--orphan` / `--no-parent` — only top-level issues (no parent).

All combine with each other (AND) and with existing filters.

## Acceptance criteria
- [ ] `--status` accepts a comma list and a leading `!` to negate.
- [ ] `--match` does a case-insensitive substring filter on the title.
- [ ] `--sort` orders by priority / points / id / created.
- [ ] `--blocked` shows only issues with an unmet (non-terminal) dependency.
- [ ] `--orphan` shows only issues without a parent.
- [ ] Filters compose (AND); default behavior unchanged when no flag is passed.
- [ ] Tests cover each new flag plus one combined query.

## Notes
`--blocked` is the inverse of the readiness check in #023 — share the dep-status
helper between them.
