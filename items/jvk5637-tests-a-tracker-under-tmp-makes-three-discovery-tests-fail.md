# tests: a tracker under /tmp makes three discovery tests fail

## Summary
Three `discovery` unit tests assert that a temp directory with no tracker in it produces
`no tracker found here; run \`trck init\``. They fail whenever *any* tracker directory sits
directly under `/tmp`:

```
discovery::tests::no_tracker_anywhere_says_how_to_make_one
discovery::tests::the_binarys_own_directory_is_not_a_tracker_source
discovery::source::tests::no_tracker_and_no_ref_keeps_the_original_diagnostic
    not found: "/tmp/trck-hooks-rootlevel"
```

`find_tracker` walks up from the fixture to `/tmp` and, at each ancestor, scans that
directory's *children* for a `trck.json`. `Tmp` puts its fixtures under `std::env::temp_dir()`,
so `/tmp` is always an ancestor — and any tracker parked beside them is a sibling the walk-up
finds. Setting `TMPDIR` does not help: the walk continues past it, up to `/`.

The suite creates one itself. `tests/git_hooks.rs` builds a tracker at `/tmp/trck-hooks-rootlevel`
(the name says it is deliberately at the root level), and `cargo test --all` runs test binaries
concurrently — so the failure is a **race between two of this repository's own suites**, not
something only a dirty machine sees. It reproduces on a pristine `origin/main`. A stray tracker
left in `/tmp` by anything else — a scratch directory, an abandoned run — makes it deterministic
instead of intermittent.

CI has been green because a fresh runner's `/tmp` is empty when the unit tests happen to run
first. That is scheduling luck, not isolation.

## Acceptance criteria
- [ ] `cargo test --all` passes with an unrelated tracker directory sitting at `/tmp/<name>`.
- [ ] The three tests no longer depend on what else is under the fixture's ancestors.
- [ ] `tests/git_hooks.rs` keeps whatever it was demonstrating by living at the root level, or the reason it no longer needs to is written down.
- [ ] The fix is a test-isolation fix: `find_tracker`'s sibling scan is behaviour users rely on and does not change here.

## Notes
Found while working #jgf9ktx, where a tracker another session had left at `/tmp/p95t` made all
three fail on every run. Confirmed pre-existing by exporting `origin/main` into a clean directory
and running the same tests there — same three failures, tripping over `/tmp/trck-hooks-rootlevel`
instead.

Likely shapes, cheapest first: give `Tmp` a nested root (`<temp>/trck-tests/<tag>-<pid>-<n>`) so
the fixture's siblings are only ever other fixtures; or have the three tests assert against a root
they fully control rather than one with real ancestors. Note that nesting `Tmp` alone does not fix
it if `git_hooks` keeps writing a sibling *of the nest* — both halves have to agree on where test
trackers live.

That `#jgf9ktx` had to be committed with `--no-verify` is the cost being paid here.
