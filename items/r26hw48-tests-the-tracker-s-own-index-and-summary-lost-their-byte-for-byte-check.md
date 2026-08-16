# tests: the tracker's own index and summary lost their byte-for-byte check

## Summary

Two unit tests read this repository's committed tracker and require the bytes back:

- `index::tests::real_indexes_round_trip_byte_for_byte` — `parse_index` then `render_index`
  must reproduce `index.jsonl` exactly.
- `summary::tests::the_repos_own_summary_regenerates_byte_for_byte` — `generate_summary`
  must reproduce the committed `SUMMARY.md` exactly.

Both iterate a list of trackers and `continue` past any that is absent, requiring only that
one was found. After the flip (#8d22h6x) `issues/` is absent, so both quietly fall back to
`examples/action-game` alone — 35 rows instead of 280-odd, and none of the shapes that only
turn up in a tracker someone actually uses: unicode titles, review links, resolutions, deep
epics, custom fields.

Nothing went red. That is the problem: the assertion still passes, on a tenth of the data,
and says so nowhere.

## Why it cannot just point at the branch

A unit test must not shell out to git to read a ref. It would fail for any consumer of the
crate, and in CI the `rust` job's checkout is shallow and deliberately knows nothing about
`trck-issues` — #u3s6y7h took the tracker out of that workflow on purpose, and putting it
back through a test would undo it silently.

So the check belongs where the branch is already fetched: `.github/workflows/tracker.yml`.

## What that needs

There is no verb that answers "is this tracker already canonical". `repo normalize` rewrites
`index.jsonl` in place and says how many rows changed; `summary` regenerates `SUMMARY.md`.
Neither has a mode that compares and exits non-zero, which is what a CI step wants.

Sketch: `trck repo normalize --check` (and the same for the summary, or one flag covering
both), reporting the first file that differs and exiting non-zero, writing nothing. Then the
tracker workflow runs it against `FETCH_HEAD` beside `check`, and the coverage is back —
against the real tracker, on every tracker commit, which is more often than the unit tests
managed.

## Acceptance criteria
- [ ] A verb answers "the committed index and summary are what the engine would write", writing nothing and exiting non-zero when they are not.
- [ ] `.github/workflows/tracker.yml` runs it against the pushed commit.
- [ ] Conformance covers the clean and dirty cases.
- [ ] The two unit tests say what they now cover, and stop implying they cover the big tracker.

## Notes

Found while doing the flip, not afterwards — the tests kept passing, so nothing would have
reported it. The `issues/` entry stays in both lists as a deliberate marker: it is the only
thing in the source that records that this coverage used to exist.
