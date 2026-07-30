# migrate this repo's tracker and the bundled example

## Summary
Both on-disk trackers in this repo are still laid out by status. Migrate them with the verb from
#x2exfdf — dogfooding it against ~160 real files is the acceptance test the unit tests can't be.

## Implementation

1. **Dry-run this repo's tracker:** `./trck repo migrate-layout --dry-run` — expect a list of
   every issue file with its planned `items/` destination and no filesystem change. If it dies on
   status/folder drift instead, those are pre-existing inconsistencies, not migration bugs;
   resolve the named issues first.
2. **Migrate:** `./trck repo migrate-layout` → `moved N file(s) into items/`
3. **Verify:** `./trck check` → `OK — N issues, 0 errors, …` (a pre-existing warning count is fine).
4. **Confirm git recorded pure renames:** `git status --short` should show `R` entries moving
   `issues/<status>/*.md` to `issues/items/*.md`, plus a modified `issues/SUMMARY.md`. No issue
   body content should change.
5. **Migrate the example:**
   `./trck --dir examples/action-game repo migrate-layout && ./trck --dir examples/action-game check`
6. **Regenerate the docs screenshots:** `python3 docs/gen-screenshots.py`, then check
   `git diff --stat docs/img/`. Any change there is cosmetic (the example tracker's rendering)
   and should be committed.

Commit the tracker migration separately from the example, per this repo's commit hygiene.

## Acceptance criteria
- [ ] `issues/` has no status folders; every body is in `issues/items/`
- [ ] `./trck check` passes
- [ ] `git status` shows renames only — no issue body content changed
- [ ] `examples/action-game` migrated and passing `check`
- [ ] `docs/img/*.svg` regenerated if the rendering changed

## Notes
Expect ~122 renames in this repo based on the churn measurement in the epic — that is the last
time these files ever move for a status change.

Step-by-step: `docs/plans/2026-07-30-flat-items-layout.md` (Task 5).
