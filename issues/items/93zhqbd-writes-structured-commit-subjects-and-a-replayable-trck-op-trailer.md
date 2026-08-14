# writes: structured commit subjects and a replayable Trck-Op trailer

## Summary
`git log --oneline trck-issues` becomes the tracker's changelog, so the subject is
engine-generated and structured: `new #id: title`, `done #id (fixed)`, `set #id priority=high`,
`dep #id +#id`.

The trailer is the load-bearing half. `Trck-Op: done abc1234 --resolution fixed` records the
operation itself, so a pending commit can be replayed against a tree it was not built on — at
any stacking depth, long after the verb that produced it has left memory. It doubles as the
audit log.

## Acceptance criteria
- [ ] Every mutating verb emits a subject in the documented shape and a `Trck-Op:` trailer.
- [ ] The trailer round-trips: parsing it back yields an operation equal to the one the verb ran, for every verb, under test.
- [ ] Titles containing newlines, quotes or leading dashes survive the round trip.
- [ ] An unparseable trailer is a diagnostic naming the commit, never a panic.

## Notes
Independent of the push loop — #C11 and #C12 consume this, but it can land and be tested on its own.
