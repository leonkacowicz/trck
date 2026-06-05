# Improve CLI output presentation (align/icons, color, show, SUMMARY)

## Summary
Make the human-facing output nicer without breaking agent/script consumers. All color/styling
is **gated to a TTY** (and honors `NO_COLOR`), so piped/redirected output and generated files
stay plain. `SUMMARY.md` (a persisted file) is never colored.

## Acceptance criteria
- [ ] `trck list`: dynamic column widths (no misalignment with custom statuses) + a status icon per row
- [ ] TTY-gated color: priority (high=red, low=dim), status (terminal=green, initial=dim, active=yellow), bold `#NNN`; auto-off when not a TTY or `NO_COLOR` is set
- [ ] `trck show`: human-readable aligned `key: value` (skipping empty fields), with `--json` for the raw machine form
- [ ] SUMMARY epic strip: drop the `?` milestone placeholder (only show the milestone overview when children have milestones)
- [ ] piped output stays plain (no ANSI); tests cover the plain path + the color helper

## Notes
Constraint: keep it agent-friendly — machine-readable output must be unchanged when stdout
isn't a terminal. Best evaluated by eye; iterate on palette/format after first cut.
