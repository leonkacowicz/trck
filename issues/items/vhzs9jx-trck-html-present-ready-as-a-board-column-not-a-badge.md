# trck-html: present ready as a board column, not a badge

## Summary
The board (`renderBoard`, `assets/app.js`) lays out one column per status and is the
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
- [x] The board renders five columns, `ready` first, sourced from the payload's `ready` field.
- [x] The `backlog` column excludes ready issues; the two counts sum to the backlog total.
- [x] Column headers carry counts, as today.
- [x] The board still fits its pane at the widths the layout already handles — five columns
      instead of four.
- [x] Facet filtering behaves as it does now (the board does not offer a status facet, since it
      lays out by status; `ready` must not reintroduce one).
- [x] The tree badge and the dedicated ready view agree with the new column on membership.

## Notes
**The tree badge keeps its name.** The suggestion below was to retitle it `unblocked`, on the
reading that the badge and the word `ready` would otherwise mean two things on one page. After
#gccs68j they do not: badge, column and view all render the same `ready` field, so a second word
would invent the ambiguity it was meant to remove. The tooltip is sharpened instead — "nobody has
started this and nothing blocks it", which is now the whole definition rather than half of it.

> ~~Consider retitling the tree badge from `ready` to `unblocked`. On a backlog leaf the two are
> the same thing after #gccs68j; the badge's own tooltip already says "nothing blocks this", which
> is the more precise claim and stops the word `ready` meaning two things in one page.~~

The column subtracts `ready` from *every* status column rather than from the initial one by name,
so the view never has to know which status is initial — a ready issue is in the initial one by
definition. `boardColumns` is a pure function for the same reason the other lifted helpers are:
`tests/app_js.rs` runs it under node, and the "every card in exactly one column" invariant is
what it asserts.

The ready view stays as it is — it mirrors `trck ready` including the demand ranking, which the
board deliberately does not.

Under #fkrp9dh's lineage — the board arrived in #2ytfth4 (v5).
