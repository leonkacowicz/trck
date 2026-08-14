# trck edit <id>: edit an existing body through the same path

## Summary
Today a body is edited by opening `issues/items/<id>-<slug>.md`. Once the tracker is off
the working tree that file is not there, so without this verb body edits stop being possible
rather than merely becoming different. This is the one part of the body-input tranche the
migration hard-depends on.

Mechanics are #D15's: fetch the body, temp file, `$EDITOR`, validate, commit, push. Same code,
different starting content.

## Acceptance criteria
- [ ] `trck edit <id>` opens the current body and files the result as one commit.
- [ ] `--body`/`--body-file` apply to `edit` too, so it is scriptable and testable without a TTY.
- [ ] An unmodified buffer is a no-op: no commit, no push, and it says so.
- [ ] An unknown id is the same diagnostic every other verb gives.
- [ ] Works against a ref-backed tracker as well as a directory-backed one.

## Notes
Blocks the flip (#E21). Everything else in the D tranche is an improvement; this one is a precondition.
