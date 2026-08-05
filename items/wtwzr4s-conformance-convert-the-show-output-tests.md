# conformance: convert the show output tests

## Summary
`show` prints aligned metadata then the body. Source: the `show` cases in `test_read.py`,
plus the field-display cases in `test_metadata.py` and `test_custom_fields.py`.

## Acceptance criteria
- [ ] Metadata block: key alignment, which keys appear, and the derived ones (points on a
      parent, epic marker) versus stored ones.
- [ ] The `--- body ---` separator and the body verbatim.
- [ ] Custom fields shown, including one with an empty value.
- [ ] Unknown/foreign index keys surfaced rather than dropped.
- [ ] Python originals deleted; assertion count carried over is checked.

## Notes
`show --json` belongs to the `--json` child, not here. Part of #xm6h2qn.
