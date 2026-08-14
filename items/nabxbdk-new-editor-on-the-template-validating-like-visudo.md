# new: $EDITOR on the template, validating like visudo

## Summary
With a TTY and no body flag, open `$EDITOR` on the prose template
(`src/verbs/mod.rs:TEMPLATE`) and behave the way `visudo` does:

- edit a temp copy, validate before accepting (title heading matches, slug sanity — the checks
  already in `validate/`);
- on failure, re-open with the error at the top rather than discarding the work;
- an empty or unmodified buffer **aborts**: no issue filed, nothing committed, nothing pushed.

visudo's lock does not transfer — contention here is on a remote ref, and #C11's retry is the
equivalent, with no stale lock to clean up.

## Acceptance criteria
- [ ] A valid buffer files the issue; an invalid one re-opens with the diagnostic at the top and the operator's text intact.
- [ ] An empty or unmodified buffer aborts with a message and creates nothing.
- [ ] `$EDITOR`, then `$VISUAL`, then a documented fallback; an editor that exits non-zero aborts.
- [ ] The temp file is removed on every path, including abort and editor failure.

## Notes
The abort-on-unmodified rule is exactly why #D14's `--empty` has to exist: a deliberate title-only issue needs a way to say so.
