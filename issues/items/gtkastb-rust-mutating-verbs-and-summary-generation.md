# rust: mutating verbs and SUMMARY generation

## Summary
`new`, `set`, `mv` and its aliases, `dep`, `label` — everything that writes — plus the
`SUMMARY.md` regeneration every mutation triggers.

## Acceptance criteria
- [ ] Every mutating verb, with the same guards: cycle rejection on re-parent and on `dep`,
      derived parent status, `--auto` returning a pinned status to derivation.
- [ ] Writes are atomic enough that an interrupted run cannot leave a half-written index.
- [ ] `SUMMARY.md` byte-identical to the Python engine's.
- [ ] Filenames follow the slug, and a title change renames.
- [ ] Passes the `av3efth` fixtures.
