# list: extract shared row renderer with connector prefix

## Summary

Phase 2 of #031 (first child past the Graph seam). A no-op refactor: extract `print_rows`
into the single shared renderer that #031 needs for both flat and nested output, taking a
connector `prefix` argument. Pass `""` at every call site for now so flat output stays
byte-identical. Phase 4 (#038) supplies real connectors for the nested path.

Row layout is unchanged: `icon · #id · status · priority · <prefix>title · tags ·
annotations`, with the prefix sitting immediately before the title.

## Acceptance criteria
- [ ] The renderer accepts a per-row connector `prefix` (default/`""` = today's flat output).
- [ ] Column-width computation (status/priority) lives in the one renderer.
- [ ] All current `list` / `ready` output is byte-identical (existing tests stay green).
- [ ] No nesting behavior yet — this is purely the extraction.
- [ ] `trck check` passes.

## Notes

Independent of #037 in principle (renderer vs. traversal), but kept serial per the
chosen linear order. Consumed by #038.
