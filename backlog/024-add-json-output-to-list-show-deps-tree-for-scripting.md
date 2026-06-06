# Add --json output to list/show/deps/tree for scripting

## Summary
Make trck a scriptable substrate (CI gates, dashboards, agents) by emitting
machine-readable output. The data is already structured in `index.jsonl`; a
`--json` flag on the read commands exposes it without new state.

- `list --json` → array of issue objects (the matched rows).
- `show --json` → the issue object plus its body text.
- `deps --json` → `{ requires: [...], blocks: [...] }`.
- `tree --json` → nested children structure.

Output goes to stdout as a single JSON document; default human output is unchanged.

## Acceptance criteria
- [ ] `--json` accepted by `list`, `show`, `deps`, `tree`.
- [ ] Output is valid JSON (one document), stdlib `json` only.
- [ ] `list --json` honors all existing filters and returns an array (possibly empty).
- [ ] `show --json` includes metadata and the raw body.
- [ ] Field names/shape are stable and documented in the command help.
- [ ] Tests assert parseable JSON and key fields for each command.

## Notes
Reuse the in-memory issue representation rather than re-reading files. Keep the
schema close to the index row so consumers can rely on it.
