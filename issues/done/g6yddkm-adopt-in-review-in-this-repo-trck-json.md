# adopt in-review in this repo trck.json

## Summary
Dogfood it: add the `in-review` status and the `review` alias to `issues/trck.json`.
`load_config` replaces top-level keys wholesale, so this repo's `statuses` list and
`aliases` map must both be edited — inheriting the new default is not automatic.

## Acceptance criteria
- [ ] `issues/trck.json` lists `in-review` with `"actionable": false`, and the alias
- [ ] `trck check` passes; `SUMMARY.md` regenerates with the new counts row
- [ ] `./trck review <id>` works in this repo

## Notes
Do this last, once the engine supports it — the config is read by the same `./trck` the
repo runs.
