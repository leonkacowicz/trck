# list: argparse/help/README and acceptance tests for the merged verb

## Summary

Phase 5 of #031 — the closing polish for the merged browse verb. Wire up the surface and
documentation, and land the full acceptance-test pass against #031's criteria.

- argparse: add `--flat` and the optional positional `id` to `list`; make `tree` an alias
  block that forwards to `list`.
- help text for `list` / `tree` updated to describe the nested default and `--flat`.
- README updated (browse-verb section, examples).
- acceptance tests covering each #031 criterion end to end.

## Acceptance criteria
- [ ] `list` argparse exposes `--flat` and a positional `id`; `tree` forwards to `list`.
- [ ] `list` / `tree` help text reflects the nested-by-default behavior.
- [ ] README documents the merged verb and shows the nested/flat examples.
- [ ] Acceptance tests cover every #031 checkbox; `trck check` passes.

## Notes

Final child of #031 — the epic is done when this closes.
