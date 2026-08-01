# list: --since/--until date-range filters over created/started/closed

## Summary
`list` can filter by status, priority, label, kind, parent, title match and custom field, but not
by time — even though every row carries `created`, `started` and `closed`. Add a date-range filter
so "what did we close last week", "what's been sitting in backlog since May" and "what got created
during the migration" are one command.

Shape: a field selector plus a range, rather than three flag pairs.

```
trck list --date closed --since 2026-06-01 --until 2026-07-01
trck list --date created --since 2026-06-01          # --date defaults to created
```

Comparison is the plain ISO string compare `changelog` already uses (`cmd_maint.py:156`) — the
stored format sorts lexicographically, so no date parsing is needed on the row side. `--since` is
inclusive, `--until` exclusive; a row with the selected field unset is excluded whenever either
bound is given.

Composes with the existing filters (AND-ed, like `--field`), and works under `--flat` and the
nested default alike.

## Acceptance criteria
- [ ] `--date {created,started,closed}` (default `created`) with `--since` / `--until`, either
      bound optional.
- [ ] Bounds go through the shared cutoff parser, so a bare date and a full timestamp both work.
- [ ] Rows missing the selected timestamp are excluded when a bound is present.
- [ ] Composes with `--status` / `--label` / `--field` / `--match` and with `--flat`.
- [ ] Help text and README examples updated; tests cover each field, each bound, both bounds, and
      the missing-value case.

## Notes
- Filter application lives in `cmd_query.py` (see the `--field` filter at `cmd_query.py:31` for
  the pattern to follow).
- Relative bounds (`--since 7d`) come free once [[ycg7egx]] lands; ship absolute-only if it hasn't.
- Deliberately *not* touching `SUMMARY.md` — this is a query-time filter only.
