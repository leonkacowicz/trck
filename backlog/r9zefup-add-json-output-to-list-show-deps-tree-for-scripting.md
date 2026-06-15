# Add --json output to list/show/deps/tree for scripting

## Summary
Make trck a scriptable substrate (CI gates, dashboards, agents) by emitting
machine-readable output. The data is already structured in `index.jsonl`; a
`--json` flag on the read commands exposes it without new state. Output goes to
stdout as a single JSON document; default human output is unchanged.

This epic is delivered by its children:

- **#059** — drop the `tree` alias (nested forest is already `list`'s default), so
  the JSON story is `list --json` (nested) / `list --flat --json` (flat) with no
  separate `tree` shape.
- **#060** — shared `emit_json` stdout seam + `--json` flag plumbing (blocks the rest).
- **#061** — `list --json` → nested hierarchy; `list --flat --json` → flat array.
- **#062** — `show --json` → single document, `{...metadata, "body": ...}`.
- **#063** — `deps --json` → `{ requires: [...], blocks: [...] }`.

## Acceptance criteria
- [ ] `--json` accepted by `list`, `show`, `deps` (`tree` dropped per #059).
- [ ] Output is valid JSON (one document per invocation), stdlib `json` only.
- [ ] `list --json` honors all existing filters; nested by default, flat under `--flat`.
- [ ] `show --json` includes metadata and the raw body in one object.
- [ ] Field names/shape are stable and documented in each command's help.
- [ ] Tests assert parseable JSON and key fields for each command.

## Notes
Reuse the in-memory issue representation (`Issue.to_dict()`) rather than
re-reading files. Keep the schema close to the index row so consumers can rely on
it. The `tree --json` line from the original spec is folded into `list --json`
(nested) now that the `tree` alias is being removed.
