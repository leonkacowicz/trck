# conformance: convert the show output tests

## Summary
`show` prints aligned metadata then the body. Source: the `show` cases in `test_read.py`,
plus the field-display cases in `test_metadata.py` and `test_custom_fields.py`.

## Acceptance criteria
- [x] Metadata block: key alignment, which keys appear, and the derived ones (points on a
      parent, epic marker) versus stored ones.
- [x] The `--- body ---` separator and the body verbatim.
- [x] Custom fields shown, including one with an empty value.
- [x] Unknown/foreign index keys surfaced rather than dropped.
- [x] Python originals deleted; assertion count carried over is checked.

## Notes
10 fixtures; both engines agree. Ratchet 176 -> 184. **Retired:** 6 Python tests / 12 assertions.

**There is no epic marker in `show`.** The criterion mentioned one, but `show` prints no `[EPIC]`
tag — that is a `list`/`SUMMARY` affordance. What `show` actually derives on a parent is the
*omission of `points`*, which is covered (`show-omits-points-on-a-parent`). Nothing to convert for
the marker.

**Three divergences found, all fixed.**
- *The reference engine was the broken one:* `trck show` on a row whose body file is missing threw
  an uncaught `FileNotFoundError` traceback, on both the human and `--json` paths. The Rust port
  already produced a clean diagnostic. Guarded in `cmd_query.cmd_show` with the wording
  `move_issue` had been using all along.
- *Port, `show`:* an empty-string custom field was omitted entirely, hiding a field that exists in
  the index and that `set --field note=` puts there on purpose.
- *Port, `list --field note=`:* the same accessor made the filter match nothing.

  The last two share a root cause: `field_value` folds "unset" and "empty" together. That is right
  for a `--show-field` column and wrong elsewhere, so `field_value_raw` now distinguishes them and
  `show` plus the `--field` filter use it. Python draws the same line — it skips only
  None/[]/False.

**Left for other children:** `show --json` (#t84am5s), and the id-prefix highlight, which asserts
bold/dim and so cannot live in a suite that runs `NO_COLOR`.
