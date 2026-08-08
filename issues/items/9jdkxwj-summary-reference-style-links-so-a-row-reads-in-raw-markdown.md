# summary: reference-style links so a row reads in raw markdown

## Summary
Every row is an inline markdown link whose URL restates its own title as a slug:

    - [x] [#82an2dy release v0.23.0 with the breaking layout change](items/82an2dy-release-v0-23-0-with-the-breaking-layout-change.md)

Roughly 70 characters of path per row, duplicating the text right next to it. Rendered on GitHub it
is invisible; in an editor — which is where the file is actually read — it is most of the line.
**Median line width is 162 characters.**

Reference-style links move the paths to a table at the bottom and leave the rows readable:

    - [ ] [#82an2dy][82an2dy] release v0.23.0 with the breaking layout change

    [82an2dy]: items/82an2dy-release-v0-23-0-with-the-breaking-layout-change.md

Measured on the sketch: **median content width 162 → 92**, with the link table adding one line per
issue at the end of the file. Rendered output is identical.

Note the near-miss: linking *only* the id and leaving the title as plain text does **not** help —
the URL is still on the line, and the median stays at 162. The fix has to move the path off the row,
not shrink the link text.

## Acceptance criteria
- [ ] Rows use `[#id][id]` reference links; the definition table is emitted once at the end of the
      file, one line per issue that appears.
- [ ] Rendered output is unchanged — same link targets, same anchor text.
- [ ] Only issues actually referenced get a definition, so a collapsed Done section does not drag the
      whole index into the table.
- [ ] Existing `expected.SUMMARY.md` goldens updated in the same change.

## Notes
- `src/summary.rs` builds the links; the path comes from the same `items/<id>-<slug>.md` rule
  `issue_path` uses, so both sides must stay on one helper.
- Independent of the other three children — it can land first or last.
- The merge driver regenerates `SUMMARY.md` rather than merging it, so a link table at the end costs
  nothing in conflicts.
