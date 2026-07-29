# docs: demand ranking in README, help epilog, AGENTS scaffold

## Summary
Document how `ready`/`next` order their output, so the marker is readable and the
priority field's role is not overstated.

## Acceptance criteria
- [ ] README: `ready`/`next` rank by demand cone; what `↑<priority>(#id)` means
- [ ] README priority section: `list --sort priority` still sorts the declared field
- [ ] `cli.py` help epilog mentions the ranking on the `ready`/`next` lines
- [ ] `templates.py` AGENTS scaffold + `issues/CLAUDE.md` note it under priorities
- [ ] `trck ready -h` / `next -h` descriptions updated

## Notes
Keep the priority-vs-dependency rule of thumb intact — demand ranking is a soft
ordering derived from hard ordering, not a replacement for either.
