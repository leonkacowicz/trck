# list: show points-weighted percent-complete on parent rows

## Summary
`trck list` shows only a status icon for parent rows. The points-weighted completion
rollup added in #019 lives solely in `SUMMARY.md` (the `### … — {pct}% (… pts · … done)`
heading). Surface that same derived percentage in `list` so progress is visible while
browsing, without opening `SUMMARY.md`.

Reuse the existing recursive leaf-tally helper from #019 (the one that feeds
`generate_summary`) — this is a display concern only, no change to the rollup math or to
how `points` is stored.

## Acceptance criteria
- [x] Parent rows in `trck list` show a points-weighted completion figure (e.g. `42%`),
      derived from leaf descendants via the same helper that backs `generate_summary`
      (new `progress_pct` wraps `leaf_rollup`).
- [x] Leaf rows show no percentage (nothing to roll up).
- [x] The forest/nested default and `--flat` both render the figure consistently; the
      row renderer (`print_rows`) stays the single source of layout — a new `progress`
      callback places the suffix right after the title in all of its branches.
- [x] Alignment/columns stay readable: the figure is a dim trailing suffix after the
      title (already a variable-width region), so id/status/priority columns are untouched.
- [x] **Decided: always-on**, not a flag — it mirrors the always-on dim blocking notes
      (`needs/blocks`) already on every row, and the line already carries variable
      trailing content (tags, annotations), so scripts keying on the fixed leading
      columns are unaffected. Documented in the `list` help and README.
- [x] Tests (`tests/test_list_progress.py`): a parent with mixed-status leaf descendants
      reports the expected pct; deep (grandchild) leaves are summed; points-weighting
      diverges from a plain count; `ptotal == 0` guard yields `0%`; leaves render no figure.

## Notes
- Split out of the points rollup work in #019, which deliberately scoped the derived
  number to `SUMMARY.md` ("the summary owns the derived number"). This issue extends that
  display to `list`.
- Status rollup (#067) is a separate, already-done concern (parent *status* from children),
  not points/percent.
- Consider the same treatment for `show` later — out of scope here.
