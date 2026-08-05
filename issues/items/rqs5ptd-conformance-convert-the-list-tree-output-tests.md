# conformance: convert the list/tree output tests

## Summary
The forest and the flat list are what most users read most often. Source suites:
`test_read.py` (the `list`/`listing` cases), `test_list_default_filter.py` (7 tests),
`test_list_progress.py` (5), and the presentation cases in `test_presentation.py`.

## Acceptance criteria
- [ ] Filters preserved: `--status` (comma-lists and `!` negation), `--priority`, `--label`,
      `--kind`, `--parent`, `--match`, `--blocked`, `--orphan`, `--field`.
- [ ] Flags preserved: `--flat`, `--all`, `--sort`, `--show-field`, `--paths`, and a root `ID`.
- [ ] The default prune (settled work hidden; open ancestors pulled back as context) and
      `--all` bypassing it.
- [ ] Nesting, the rolled-up parent percent, and the dim `needs`/`blocks` annotations.
- [ ] The Python originals are deleted, not left alongside; assertion count carried over
      is checked, not assumed.

## Notes
Golden stdout is strictly stronger than the `assertIn` checks it replaces, so one fixture can
retire several assertions — but every distinct (setup, flag) scenario must still be represented.
Part of #xm6h2qn.
