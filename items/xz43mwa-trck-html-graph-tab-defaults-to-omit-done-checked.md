# trck-html: graph tab defaults to 'omit done' checked

## Summary

The graph tab opens with `omit done` unchecked, so the first thing it draws is the whole
history: every closed issue and every edge into one. On a tracker with a few hundred settled
rows that is a wall, and the useful graph — what is still open and what blocks it — only
appears after the reader finds the checkbox and unticks it. Start from the answer people
want and let them switch the finished work back on.

`state.graphOmitDone` starts `false` in `assets/app.js:33`; the checkbox that reads it is
built at `assets/app.js:392`, and the filter it drives runs at `assets/app.js:856`.

## Acceptance criteria
- [ ] The graph tab renders with done issues hidden on first open, with the `omit done` box checked.
- [ ] Unchecking it brings the done work back exactly as it does today — the filtering logic is unchanged, only its initial value.
- [ ] The other tabs are untouched; `graphIncludeDone` keeps its current default.
- [ ] Covered by a test in `tests/app_js.rs` if the default is reachable from a pure function, otherwise by a conformance fixture over the emitted asset.

## Notes

Only the initial value moves. `graphIncludeDone` (the done-chain toggle) is a separate
control and is deliberately left alone here.
