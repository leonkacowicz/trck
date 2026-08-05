# conformance: convert the mutating-verb output tests

## Summary
What the mutating verbs echo to stdout, which is as much a contract as the read verbs: `new`
(prints the created path), `mv`/`start`/`review`/`done`, `set`, `dep`, `label`. Sources:
`test_metadata.py` (30), `test_labels.py` (14), `test_lifecycle.py` (7), `test_review.py` (33),
`test_custom_fields.py` (24) — the stdout-asserting cases in each.

## Acceptance criteria
- [ ] Each verb's echo line, including the created path from `new`.
- [ ] `review` recording the PR URL and moving status in one step; `done --resolution …`.
- [ ] `label --add/--remove` sorted-and-echoed; `set --field`/`--unset`.
- [ ] Derived-status behaviour on a parent (`--auto`, and refusing a hand-set status).
- [ ] Python originals deleted; assertion count carried over is checked.

## Notes
Some of these already have a fixture (`label-add-remove-is-sorted-and-echoed`,
`done-stamps-closed-and-satisfies-dependents`); extend rather than duplicate. Part of #xm6h2qn.
