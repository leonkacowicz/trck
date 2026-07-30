# v5: board / kanban by status

## Summary

Add a fourth view to the SPA: a **board / kanban**. The toggle becomes
`[ list | graph | tree | board ]`. One column per configured status (in `trck.json`
order), each holding the issues in that status as cards. Cards click through to the
detail panel; the filter bar narrows the cards.

## Design

**Data:** none new — the board is a pure re-layout of existing data. Columns come from
`config.statuses` (already embedded, in order); cards are `issues` grouped by `status`.
So the only Python-testable addition is the presence of the board UI; the status/vocab
data it consumes is already covered.

**View (JS; browser-verified):**
- A `board` button joins the view toggle; selecting it shows the board in the left pane
  (columns flex horizontally, each scrolls vertically; the board scrolls horizontally if
  the columns overflow).
- One column per `config.statuses` entry: header = status name + card count; body =
  cards for issues in that status.
- A card shows `#id`, title, a priority badge, a rolled-up `pct%` for parents, and the
  v2 "edited" marker. Clicking a card → `select(id)` (detail updates, card highlighted).
- Filter-aware (consistent with list/tree): `matches()` decides which cards show; columns
  always render with their filtered counts. Changing a filter re-renders the board.

**Deferred (call out, don't build yet):** drag-a-card-to-another-column to *stage* a
`mv` status change (would pair with v2 command-copy) — a natural follow-up once the
static board looks right. No persistence in a static file regardless.

## Acceptance criteria

- [ ] Rendered document includes the board container + the `board` view toggle.
- [ ] Board renders one column per configured status, in order, with per-column counts.
- [ ] Cards click through to the detail panel; the filter bar narrows the cards.
- [ ] v1–v4 behaviour unchanged; full suite + `build.py --check` green.

## Notes

The board is view-only JS over existing data, so Python tests cover the board-UI presence
(the vocabulary/grouping data is already tested); layout + interaction are browser-verified.
Drag-to-stage is deliberately deferred. Parent: epic #fkrp9dh.
