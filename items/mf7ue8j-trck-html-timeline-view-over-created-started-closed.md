# trck-html: timeline view over created/started/closed

## Summary
The browser tool already exports all three timestamps into its JSON payload (`tools/trck-html`,
the row builder around `"created" / "started" / "closed"`) and renders none of them. A timeline
view is therefore almost pure front-end work — no engine change, no new export.

The view: a horizontal time axis with one row per issue, each drawn as a bar from `created` to
`closed` (or to now, if open), with a tick where `started` falls — so the gap before work began
reads as visibly distinct from the work itself. Grouped by epic, it becomes a plain progress
picture of a subtree; filtered to a date range, it becomes "what happened in June".

It should reuse what the tool already has rather than growing a parallel world: the existing
filter/search state selects which issues appear (as the graph view's filter-as-seeds does,
#zssaj4k), clicking a bar opens the same detail panel, and the shortest-unique-id prefix
highlighting (#d9ckqzc) carries over.

## Acceptance criteria
- [ ] A timeline view alongside the existing list and graph views, sharing their filter state.
- [ ] Each issue renders as a created→closed span with a `started` marker; open issues run to now.
- [ ] Clicking an issue opens the existing detail panel.
- [ ] Sensible handling of the degenerate cases: same-day spans, missing `started`, a single issue,
      and a range spanning years.
- [ ] Axis labels adapt to the visible range (days vs. weeks vs. months).
- [ ] The tool stays a single self-contained HTML file with no external assets.

## Notes
- Data is already exported — the timestamps go into the payload unused today.
- Same-day issues are the common case in this tracker; a naive linear scale collapses them to
  zero-width. Enforce a minimum bar width.
- Grouping by epic wants the hierarchy the tool already loads; ordering within a group by `created`
  is the obvious default.
