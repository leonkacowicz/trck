# migrate the bundled example and dogfood migrate-layout

## Summary
Migrate `examples/action-game` — the last tracker in the repo still laid out by status folder —
using the verb from #x2exfdf. This is also the only remaining chance to dogfood that verb against
real data, so it doubles as the verb's acceptance test.

> **Scope amended during #v7zzefd.** This repo's own `issues/` tree was **already migrated**, in
> the #v7zzefd commit (all 126 files, via `git mv`; `check` green). The engine and the tracker
> data have to agree at every commit — the pre-commit hook runs `trck check`, so deferring the
> data migration to this issue would have left `main` red across four commits and made every
> `trck` verb unusable in the repo meanwhile. What was left here is the example tracker.

## Implementation

1. **Dry-run:** `./trck --dir examples/action-game repo migrate-layout --dry-run` — expect a list
   of every issue file with its planned `items/` destination and no filesystem change. If it dies
   on status/folder drift instead, those are pre-existing inconsistencies, not migration bugs;
   resolve the named issues first.
2. **Confirm the guard fires first:** `./trck --dir examples/action-game check` should produce the
   legacy-layout refusal from #8fdjhhf, naming the folders and pointing at
   `trck repo migrate-layout`. This is the only end-to-end proof the guard fires on a real
   un-migrated tracker — the unit tests use synthetic fixtures.
3. **Migrate:** `./trck --dir examples/action-game repo migrate-layout` → `moved N file(s) into items/`
4. **Verify:** `./trck --dir examples/action-game check` → `OK`
5. **Confirm git recorded pure renames:** `git status --short -- examples/` should show `R`
   entries only, plus a modified `SUMMARY.md`. No issue body content should change.
6. **Regenerate the docs screenshots:** `python3 docs/gen-screenshots.py`, then check
   `git diff --stat docs/img/`. Any change there is cosmetic (the example tracker's rendering)
   and should be committed.

## Acceptance criteria
- [ ] The legacy-layout guard is observed firing on `examples/action-game` before migration
- [ ] `examples/action-game` has no status folders; every body is in its `items/`
- [ ] `./trck --dir examples/action-game check` passes
- [ ] `git status` shows renames only — no issue body content changed
- [ ] `docs/img/*.svg` regenerated if the rendering changed

## Notes
This repo's own migration produced **126 renames** — the last time those files ever move for a
status change. Measured payoff on the first close under the new layout: 2 files changed
(`index.jsonl` + `SUMMARY.md`) versus 4 under the old layout (the same two, plus two zero-line
renames).

Step-by-step: `docs/plans/2026-07-30-flat-items-layout.md` (Task 5).
