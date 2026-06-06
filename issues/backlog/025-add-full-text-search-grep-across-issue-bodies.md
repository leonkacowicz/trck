# Add full-text search/grep across issue bodies

## Summary
`list` only filters indexed metadata; there is no way to find an issue by
something written in its prose body (Summary / Acceptance criteria / Notes).
Add `trck search <query>` (alias `grep`) that scans issue bodies and titles and
prints the matching issues, like a `list` result.

Matching is a plain substring by default, case-insensitive, with an optional
`--regex` flag. Composes with existing metadata filters (e.g. `--status`).

## Acceptance criteria
- [ ] `trck search <query>` matches against title + body text and lists hits.
- [ ] Case-insensitive substring by default; `--regex` opts into regex (stdlib `re`).
- [ ] Honors metadata filters (at least `--status`) to narrow the search set.
- [ ] Prints in the same one-line-per-issue format as `list`; empty result prints nothing.
- [ ] Tests cover: body hit, title hit, no hit, regex match, filter intersection.

## Notes
Read body text from the issue markdown files. Keep it stdlib-only — no external
grep dependency.
