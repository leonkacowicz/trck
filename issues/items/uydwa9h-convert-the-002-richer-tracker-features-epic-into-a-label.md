# Convert the #4vqukyy 'richer tracker features' epic into a label

## Summary
#4vqukyy ("Part D: richer tracker features") is used as a generic bucket of loosely related
enhancements, not a genuine decomposition. Per the tracker's own guidance (issues/CLAUDE.md),
parent/child is *decomposition* — a parent is closable exactly when all its children are
done — whereas a "category of similar things" should be a **label**. #4vqukyy fails that litmus
test, so it should become a label and its direct children should be re-homed.

## Acceptance criteria
- [ ] Introduce a label (e.g. `tracker-features`) to mark the issues currently bucketed
      under #4vqukyy.
- [ ] Re-home every direct child of #4vqukyy: clear its parent (`trck set NNN --parent none`)
      and apply the new label — except where a child is a *genuine* sub-epic in its own
      right (e.g. the `--json` epic #r9zefup and the custom-fields epic #h7xp2dm), which keep their
      own real parent/child subtrees intact and just gain the label.
- [ ] Close out #4vqukyy itself once emptied — choose `done` or a `superseded` resolution; it
      no longer represents a unit of work.
- [ ] `trck check` passes; `trck list` no longer shows the #4vqukyy forest, and the label
      filter (`trck list --label tracker-features`) surfaces the same set.

## Notes
- Direct children to re-home (as of filing): #httj4xf, #eemqu4g, #s3d6xyz, #qc48tds, #r9zefup, #5wbwpjv, #9sevgpn,
  #cea683t, #3x6mmhu, #h7xp2dm, #6ddksge, #dscmxng, #ey2aruc. Verify against `trck list` at execution time — the
  set may have shifted.
- Sub-epics (#r9zefup --json, #h7xp2dm custom-fields) keep their internal hierarchy; only their
  link to #4vqukyy changes.
- Mirrors the existing `conflict-resolution` label already applied to #6ddksge/#dscmxng/#ey2aruc.
- Pure bookkeeping — no engine code change. Keep this commit separate from engine work.
