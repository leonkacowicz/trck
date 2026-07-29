# ready: mark inferred rows with ↑<priority>(#culprit)

## Summary
Without a marker, `ready` shows a `medium` row above a `high` one and gives no reason.
Annotate every row whose demand cone outranks its own priority with the inferred
priority and the issue that drives it.

## Acceptance criteria
- [ ] rows whose cone outranks their own priority get `↑<priority>(#culprit)`
- [ ] rows whose own priority is already the cone maximum get no marker
- [ ] the culprit id uses the same shortest-unique-prefix highlighting as the row ids
- [ ] coloured like the inferred priority; plain text under `--no-color`/non-tty
- [ ] tests: marker present/absent, names the right issue, survives the plain-text path

## Notes
Trailing annotation via `print_rows(annotate=…)` — the same slot `list` already uses for
its dim `needs #NNN`/`blocks #NNN` notes, so no column-alignment surgery in the renderer
shared with `list`.
