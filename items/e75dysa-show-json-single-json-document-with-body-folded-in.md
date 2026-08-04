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
- [ ] Routed through the #v8tmkrt helper; default (non-`--json`) `show` output unchanged.
- [ ] Field shape documented in `show` help; test asserts single parseable document with metadata + `body`.

## Notes
Depends on #v8tmkrt. Handler `cmd_show` — `src/trck/cmd_query.py:12`; the current
partial branch is the `if getattr(args, "json", False)` at `cmd_query.py:20`,
followed by the unconditional `--- body ---` print at `cmd_query.py:33` — that
print must move inside the JSON object on the `--json` path (and stay where it is
on the human path). Body text = `issue_path(ctx, row).read_text()` —
`src/trck/index.py:240`.

Note the flat layout (v0.23.0) weakens this issue's unique value: the body path is
now always `items/{id}-{slug}.md` (`filename()` — `src/trck/index.py:228`, with
legacy numeric ids zero-padded to 3), derivable straight from index fields, so a
consumer can already reach the body with jq + `cat`. What `show --json` still
buys is one parseable document instead of shell glue — and fixing today's
two-documents-on-stdout bug.
