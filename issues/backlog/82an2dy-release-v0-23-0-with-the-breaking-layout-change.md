# release v0.23.0 with the breaking layout change

## Summary
Ship the flat layout. Minor-version bump rather than patch because this is a **breaking on-disk
format change**: an updated engine refuses to operate on an un-migrated tracker.

## Implementation

Per the release process in `CLAUDE.md`:

1. Bump `__version__` in **`src/trck/constants.py`** from `0.22.0` to `0.23.0`
2. `python3 build.py` (regenerate `./trck`), then `python3 build.py --check` — exits 0, no diff
3. `python3 -m unittest discover -s tests -v` — zero failures
4. `./trck check && ./trck version` → `OK — …` then `0.23.0`
5. Commit `./trck` together with the source, then tag `v0.23.0`
6. Create the GitHub Release so `trck update` picks it up on the stable channel

**The release notes must lead with the breaking change and its one-line remedy**
(`trck repo migrate-layout`). Users who update in place will hit the layout guard on their very
next command, and the note is the only place they'll look.

## Acceptance criteria
- [ ] `__version__` is `0.23.0` and `./trck version` agrees
- [ ] `python3 build.py --check` passes (engine in sync with `src/`)
- [ ] Full suite green
- [ ] `./trck check` passes
- [ ] Tagged `v0.23.0`
- [ ] GitHub Release published, leading with the breaking change and `trck repo migrate-layout`

## Notes
Step-by-step: `docs/plans/2026-07-30-flat-items-layout.md` (Task 7).
