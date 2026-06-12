# show --json: single JSON document with body folded in

## Summary
`show --json` already exists but is non-compliant: it prints a metadata JSON blob
and then dumps the body after a `--- body ---` separator — two documents on stdout,
not one. Rework it to emit a single JSON object with the body folded in as a field,
so the whole output is parseable as one document.

- `show NNN --json` → `{...metadata, "body": "<raw markdown body>"}`.

## Acceptance criteria
- [ ] Output is exactly one JSON document (no `--- body ---` text, no trailing prose).
- [ ] Object includes the issue metadata (current `show` key selection) plus a `body` string holding the raw file body.
- [ ] Non-leaf `points` handling matches today's human `show` (points omitted where it's derived, not an input).
- [ ] Routed through the #060 helper; default (non-`--json`) `show` output unchanged.
- [ ] Field shape documented in `show` help; test asserts single parseable document with metadata + `body`.

## Notes
Depends on #060. Handler `cmd_show` ~line 1404; the current partial branch is the
`if getattr(args, "json", False)` at ~line 1412 followed by the unconditional
`--- body ---` print — that body print must become part of the JSON object in the
`--json` path. Body text = `issue_path(ctx, row).read_text()` (what `show` already
reads).
