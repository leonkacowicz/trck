# Allow any issue to be a parent (rollup for any parent)

## Summary
Relax the rule that a `parent` must be an issue of `kind: epic`. Any existing issue may be a
parent (subtasks at any depth; the engine's tree/cycle handling already supports this). The
`% done` rollup is computed for **any issue that has children**, not only epics. `kind: epic`
remains a display label/tag, not a structural constraint.

## Acceptance criteria
- [ ] `validate` no longer errors on a non-epic parent (still requires parent to exist; still rejects cycles)
- [ ] `cmd_new` / `cmd_set` accept any existing issue as `--parent`
- [ ] `generate_summary` shows a rollup for any issue with children (section retitled from "Epics")
- [ ] standalone status sections exclude parents and children (no double-listing)
- [ ] the "epic has no child issues" warning is dropped (an epic-labeled leaf is now valid)
- [ ] tests updated/added; full suite green

## Notes
Design decision (post-v0.1.0): chose "any issue can be a parent; rollup for any parent" over
keeping the epic-only constraint. Spec updated in the design doc. A 0.1.1 release is needed to
propagate via `trck update`.
