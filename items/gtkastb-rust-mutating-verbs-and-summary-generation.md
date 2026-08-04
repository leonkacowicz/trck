# rust: mutating verbs and SUMMARY generation

## Summary
`new`, `set`, `mv` and its aliases, `dep`, `label` — everything that writes — plus the
`SUMMARY.md` regeneration every mutation triggers.

## Acceptance criteria
- [x] Every mutating verb, with the same guards: cycle rejection on re-parent and on `dep`,
      derived parent status, `--auto` returning a pinned status to derivation.
- [x] Writes are atomic enough that an interrupted run cannot leave a half-written index.
- [x] `SUMMARY.md` byte-identical to the Python engine's.
- [x] Filenames follow the slug, and a title change renames.
- [x] Passes the fixtures that exist. 8 of 9 against the Rust binary; the one failure is
      `deps`, a read verb belonging to `bdmgj7r`. `av3efth`'s own conversion has not run
      yet, so "passes its fixtures" could not be met literally — five new fixtures cover
      the verbs instead, and the CI floor moved 0 -> 8.

## Landed
`00b362f`. `verbs.rs`, `summary.rs`, `cli.rs` — 107 Rust tests, clippy clean.

**SUMMARY.md is verified by regenerating this repo's own committed file** and requiring
the bytes back: 195 issues with real epics, labels, review links, resolutions and unicode
titles, plus the bundled example. It is generated *and* committed, so a byte difference is
a diff in someone's working tree — the strongest check available short of the differential
runner.

Three things worth recording:

**The calendar is hand-written.** std has no calendar and the engine takes no
dependencies, so `now_utc` needs days-from-civil in both directions. It is Hinnant's
published algorithm kept in its single-letter form, with the pedantic lint allowed and a
reason — renaming those bindings would make it unverifiable against the reference.

**`label` and `dep` echo a Python list literal**, so there is a `python_list` helper.
That looked like an accident of the Python implementation; it is not, because the suite
compares stdout literally and the echo is as much a contract as the index.

**Unimplemented verbs exit non-zero saying so.** A half-implemented verb producing
plausible-but-wrong output is precisely what would turn the pass rate into a lie, which is
the one thing this whole arrangement exists to prevent.
