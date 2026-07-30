# summary: include foreign/unknown statuses in the counts table

## Summary
`generate_summary`'s counts table lists only configured statuses; a row whose status isn't configured (already a `validate` error) is omitted, so the table total doesn't reconcile. Include any status actually present.

## Acceptance criteria
- [ ] counts table includes every status present in rows
- [ ] total reconciles with row count

## Notes
