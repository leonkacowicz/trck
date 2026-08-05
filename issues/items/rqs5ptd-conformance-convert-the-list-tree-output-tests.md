# conformance: convert the list/tree output tests

## Summary
The forest and the flat list are what most users read most often. Source suites:
`test_read.py` (the `list`/`tree` cases), `test_list_default_filter.py` (7 tests),
`test_list_progress.py` (5), and the presentation cases in `test_presentation.py`.

## Acceptance criteria
- [x] Filters preserved: `--status` (comma-lists and `!` negation), `--priority`, `--label`,
      `--parent`, `--match`, `--blocked`, `--orphan`, `--field`.
- [x] Flags preserved: `--flat`, `--all`, `--sort`, `--show-field`, `--paths`, and a root `ID`.
- [x] The default prune (settled work hidden; open ancestors pulled back as context) and
      `--all` bypassing it.
- [x] Nesting, the rolled-up parent percent, and the dim `needs`/`blocks` annotations.
- [x] The Python originals are deleted, not left alongside; assertion count carried over
      is checked, not assumed.

## Notes
49 fixtures; both engines agree on all of them. Ratchet 64 -> 112.

**Retired:** 37 Python tests / 79 assertions — `test_list_default_filter.py` entirely, 3 of
`test_list_progress.py`, 34 of `test_read.py`, plus helpers left dead by the removals.

**`--kind` is not a `list` filter.** The criterion inherited it from #xm6h2qn, but the flag does
not exist in either engine — the vocabulary change dropped it. Nothing to convert.

**Deliberately kept in Python**, with the reason in each case:
- `test_list_filter_dims_nonmatching_ancestor` — asserts a *dim* ancestor row; conformance runs
  `NO_COLOR`, so the property is invisible to a fixture.
- `test_list_argparse_exposes_flat_and_id`, `test_list_help_mentions_nested_and_flat` — argparse
  introspection and help text, neither a conformance target (see #xm6h2qn on help).
- The two `progress_pct` unit tests — they call the helper directly at states the CLI cannot
  produce (leaf-vs-parent return value, and a zero-point rollup). The *rendered* rollup is
  covered by fixtures.

The first pass at the inventory missed 12 tests that reach `cmd_list` through a helper rather
than in the test body; the assertion-parity check is what surfaced them. Worth repeating on the
sibling children: grep the helper bodies, not just the tests.
