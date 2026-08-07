# list: a distinct gutter glyph for ready rows

## Summary
`STATUS_ICON` in `src/trck/render.py` maps the four statuses onto three glyphs —
`○` backlog, `◐` ongoing *and* in-review, `●` done. Once `ready` is a coherent presented status
(#gccs68j), `list` can show it the same way the board does: a fourth glyph for a backlog row
that is ready, leaving plain `○` for backlog rows that are blocked or are epics.

In-place, no new column and no new flag — the same width, the same alignment, one more thing the
gutter tells you. Scanning `trck list` then answers "what could I pick up" without a second
command, which is the whole appeal of putting `ready` in the gutter rather than in a `--ready`
filter nobody would remember to pass.

Lowest-value of the four, and last: it is worth doing only if the glyph genuinely reads at a
glance in a dense list.

## Acceptance criteria
- [x] A ready row renders a distinct single-width glyph; a blocked or epic backlog row keeps `○`.
- [x] Alignment is unchanged — single-width, pinned by a test over every status × ready.
- [x] The legend (`trck list --help` and the README) explains the glyph set including the new one.
- [x] A conformance fixture pins the gutter for ready / blocked / epic backlog rows.
- [~] Two-engine agreement — moot, as in #gccs68j. There is one engine.

## Notes
**The glyph is `◇`, not `◔`.** The candidates were looked at in a terminal side by side, and
the reasoning that settled it is not really about the font: `○◐●` is a *fill gauge*, a picture
of how far along the work is, and readiness is not a point on that scale — it says the work is
available, which is a fact about its dependencies and about nobody holding it. A fourth degree
of fill would have re-introduced the "started work is nearly ready" reading the whole epic
removed. Leaving the family says it outright, and as a side effect nothing can be confused with
`◐` in a bad font. It is also the one glyph rendered bright rather than dim, since scanning for
it is the point.

`deps` deliberately keeps status-only glyphs. That view answers "what is waiting on what", and
marking the roots of the unblocked chains would restate what the gutter beside them already
draws. Worth revisiting only if the graph ever gets read as a pick-list.

The gutter glyph and its colour are now decided together (`render::colour::gutter`), because
they are one decision: the two were separate calls, and the ready case is the first row state
where the pair does not follow from the status alone.

Watch the interaction with #teawzv6 and #3dtnmtv, which both want to put a staleness or
due-date marker on `list`/`ready` rows. Those are trailing markers rather than gutter glyphs, so
they should not collide — but whichever lands first sets the convention for the other.
