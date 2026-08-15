# Part C: curses TUI for browsing and quick actions

## Summary
A full-screen terminal UI to browse issues, view the epic/dependency tree, and trigger quick actions (status moves, open `$EDITOR` for bodies) on top of the existing engine verbs. Deferred from the A+B spec; needs its own design (Part C).

## Acceptance criteria
- [ ] Read-only browser: list/filter issues, view tree & deps
- [ ] Quick actions delegate to engine verbs (mv/start/done/set)
- [ ] Edit issue prose via `$EDITOR`
- [ ] Ships inside the one binary — nothing extra to install at run time

## Notes
Engine verbs already exist; the TUI is a frontend over them. Write a Part C design spec first.

"curses, Python stdlib only" was the original framing and is dead: the engine is Rust, and the
dependency rule is now "one self-contained binary", so a terminal-UI crate is fair game as long
as it links statically. Whether to take one or drive the terminal directly is a design question
for the Part C spec, not a constraint set here.
