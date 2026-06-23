# Convert the #002 'richer tracker features' epic into a label

## Summary
#002 ("Part D: richer tracker features") is used as a generic bucket of loosely related
enhancements, not a genuine decomposition. Per the tracker's own guidance (issues/CLAUDE.md),
parent/child is *decomposition* — a parent is closable exactly when all its children are
done — whereas a "category of similar things" should be a **label**. #002 fails that litmus
test, so it should become a label and its direct children should be re-homed.

## Acceptance criteria
- [ ] Introduce a label (e.g. `tracker-features`) to mark the issues currently bucketed
      under #002.
- [ ] Re-home every direct child of #002: clear its parent (`trck set NNN --parent none`)
      and apply the new label — except where a child is a *genuine* sub-epic in its own
      right (e.g. the `--json` epic #024 and the custom-fields epic #048), which keep their
      own real parent/child subtrees intact and just gain the label.
- [ ] Close out #002 itself once emptied — choose `done` or a `superseded` resolution; it
      no longer represents a unit of work.
- [ ] `trck check` passes; `trck list` no longer shows the #002 forest, and the label
      filter (`trck list --label tracker-features`) surfaces the same set.

## Notes
- Direct children to re-home (as of filing): #003, #004, #005, #023, #024, #025, #026,
  #028, #030, #048, #064, #065, #066. Verify against `trck list` at execution time — the
  set may have shifted.
- Sub-epics (#024 --json, #048 custom-fields) keep their internal hierarchy; only their
  link to #002 changes.
- Mirrors the existing `conflict-resolution` label already applied to #064/#065/#066.
- Pure bookkeeping — no engine code change. Keep this commit separate from engine work.
