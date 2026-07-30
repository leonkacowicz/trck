# trck-html: export and link the pr field

## Summary
Carry `pr` through `tools/trck-html`: add it to `_issue_dict`, render it as a real
anchor in the detail pane, and mark a row that has one so a PR is findable from the
list without opening each issue.

The status side needs nothing — the board, filters, and badges are already built from
`config.statuses`, so `in-review` appears on its own merits.

## Acceptance criteria
- [ ] `pr` present in the exported issue JSON
- [ ] Detail pane renders an `<a href>` to the PR (escaped, `rel="noreferrer"`)
- [ ] Rows carry a PR marker only when set
- [ ] `in-review` gets its own board column with no code change

## Notes
Keep it display-only; the staged-edit mechanism stays status/priority.
