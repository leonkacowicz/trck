# Part C: curses TUI for browsing and quick actions

## Summary
A curses-based TUI (Python stdlib only) to browse issues, view the epic/dependency tree, and trigger quick actions (status moves, open `$EDITOR` for bodies) on top of the existing engine verbs. Deferred from the A+B spec; needs its own design (Part C).

## Acceptance criteria
- [ ] Read-only browser: list/filter issues, view tree & deps
- [ ] Quick actions delegate to engine verbs (mv/start/done/set)
- [ ] Edit issue prose via `$EDITOR`
- [ ] Stays stdlib-only (`curses`), zero-dependency

## Notes
Engine verbs already exist; the TUI is a frontend over them. Write a Part C design spec first.
