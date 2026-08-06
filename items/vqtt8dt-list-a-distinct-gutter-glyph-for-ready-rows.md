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
- [ ] A ready row renders a distinct single-width glyph; a blocked or epic backlog row keeps `○`.
- [ ] Alignment is unchanged — single-width, as the existing comment on `STATUS_ICON` requires.
- [ ] Both engines render the same glyph for the same row.
- [ ] The legend (`trck --help` / docs) explains the glyph set including the new one.
- [ ] A conformance fixture pins the gutter for ready / blocked / epic backlog rows.

## Notes
`◔` is the obvious candidate but the choice deserves a look in a real terminal at a few fonts
before it is pinned by a fixture — `◐` and `◔` are close enough to confuse if the font renders
them poorly.

Watch the interaction with #teawzv6 and #3dtnmtv, which both want to put a staleness or
due-date marker on `list`/`ready` rows. Those are trailing markers rather than gutter glyphs, so
they should not collide — but whichever lands first sets the convention for the other.
