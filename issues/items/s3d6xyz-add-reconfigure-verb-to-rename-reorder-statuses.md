# Add reconfigure verb to rename/reorder statuses

## Summary
A `reconfigure` verb to rename/reorder statuses safely: it must move issue files between renamed status folders and update the index. Acknowledged-but-deferred in the A+B spec (§11).

## Acceptance criteria
- [ ] rename a status (moves folder + updates rows)
- [ ] reorder statuses
- [ ] dry-run; `check` passes afterward

## Notes
