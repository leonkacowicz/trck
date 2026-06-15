# json: shared emit_json stdout seam + --json flag plumbing

## Summary
Foundation for the `--json` work (parent #024). Add one small helper that all the
read commands route their machine-readable output through, so JSON encoding,
options, and trailing-newline behaviour are identical everywhere — and add the
`--json` flag to the subparsers that will grow it (`list`, `show`, `deps`). The
per-command payloads land in their own issues (#061/#062/#063); this issue is just
the shared seam so those can be small and consistent.

## Acceptance criteria
- [ ] A single helper (e.g. `emit_json(obj)`) serializes to stdout with `json.dumps(obj, ensure_ascii=False, indent=2)` and a trailing newline, stdlib `json` only.
- [ ] `--json` flag declared on the `list`, `show`, and `deps` subparsers (store_true).
- [ ] Default (no `--json`) human output for all three is byte-for-byte unchanged.
- [ ] Serialization of an issue reuses `Issue.to_dict()` — no re-reading files, no bespoke field lists.
- [ ] A test covers the helper's output (valid JSON, expected shape) directly.

## Notes
Blocks #061/#062/#063. `Issue.to_dict()` (`trck` ~line 333) is the canonical
mapping (canonical keys + `extra`) and should be the basis for every emitted issue
object so the schema stays close to the index row. `show` already has a partial
`--json` branch (~line 1412) that #062 will rework — this issue can leave it as-is
or move it onto the new helper.
