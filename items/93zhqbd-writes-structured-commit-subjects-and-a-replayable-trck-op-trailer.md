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
- [x] Every mutating verb emits a subject in the documented shape and a `Trck-Op:` trailer.
- [x] The trailer round-trips: parsing it back yields an operation equal to the one the verb ran, for every verb, under test.
- [x] Titles containing newlines, quotes or leading dashes survive the round trip.
- [x] An unparseable trailer is a diagnostic naming the commit, never a panic.

## Notes
Independent of the push loop — #C11 and #C12 consume this, but it can land and be tested on its own.

Landed in PR #41. `src/verbs/op/{mod,parse}.rs` renders and reads the operation;
`src/verbs/backend/message/{mod,subject}.rs` builds the commit. The seams are where they are
because the halves fail differently: rendering cannot fail and reading can, and a subject is prose
while a trailer is data.

**Writing the round trip as a property found two real bugs** in the quoting #yuj6azz shipped, both
silent: a value beginning with `-` rendered bare and read back as a *flag*, and a newline was
embedded literally — correct in isolation, but a trailer is one line, so everything past the break
landed in what git reads as the next paragraph. Line breaks are escaped now, with a test that the
rendering never contains one.

A commit with **no** trailer is `None`, not an error: the branch can hold commits this engine did
not write. One that is present and will not parse is an error naming the fault. The last trailer
wins, so a squash that stacks messages still reports what this commit did.

The subject fallback names the issue an op acted on rather than printing the verb alone — `edit`
arrived from #zxz9vu2 mid-flight and would otherwise have silently lost its id.

**Open question for #5w9d7sq:** neither `new`'s op nor `edit`'s records the body, so an op replayed
on its own produces an issue with no prose. Nothing is lost today because the bytes are in the
changeset, but replay should decide whether the trailer needs to carry it — putting a body in every
such commit message has a real size cost.

Not covered: a title with a *leading* dash cannot be typed, because the CLI has no `--` separator.
`Op` handles it and is tested for it; the CLI gap is real but is not this issue's.
