# trck-html: present ready as a board column, not a badge

## Summary
The board (`renderBoard`, `crates/trck/assets/app.js`) lays out one column per status and is the
one view that drops the `ready` badge the tree renders. Rather than add the badge there, give
`ready` a column: it is the view organised *by* status, and after #gccs68j `ready` is a strict
subset of `backlog`, so a column steals from exactly one place and every card still sits in
exactly one column.

```
ready | backlog | ongoing | in-review | done
```

`ready` holds unblocked backlog leaves; `backlog` keeps the rest — blocked leaves and epics —
and reads naturally as "not yet". Nothing `ongoing` can claim to be ready, which was the
confusion that started this.

Presentation only. No status is stored, no card can be dragged into `ready`, and the column is
derived from the `ready` field the payload already carries.

## Acceptance criteria
- [ ] The board renders five columns, `ready` first, sourced from the payload's `ready` field.
- [ ] The `backlog` column excludes ready issues; the two counts sum to the backlog total.
- [ ] Column headers carry counts, as today.
- [ ] The board still fits its pane at the widths the layout already handles — five columns
      instead of four.
- [ ] Facet filtering behaves as it does now (the board does not offer a status facet, since it
      lays out by status; `ready` must not reintroduce one).
- [ ] The tree badge and the dedicated ready view agree with the new column on membership.

## Notes
Consider retitling the tree badge from `ready` to `unblocked`. On a backlog leaf the two are the
same thing after #gccs68j; the badge's own tooltip already says "nothing blocks this", which is
the more precise claim and stops the word `ready` meaning two things in one page.

The ready view stays as it is — it mirrors `trck ready` including the demand ranking, which the
board deliberately does not.

Under #fkrp9dh's lineage — the board arrived in #2ytfth4 (v5).
