# serve: pending and sync in the UI — unpushed commits visible and flushable

## Summary
A write whose push was rejected leaves the local ref ahead: the work is safe, but only a printed
line says so — and in a browser there is no line. `serve` is the first place pending state has
somewhere to live.

## Acceptance criteria
- [ ] The page shows how many commits are pending, and clears the indicator when they land.
- [ ] `sync` is reachable from the page, flushes the pending commits, and reports what happened.
- [ ] An `Op` that no longer applies against the new tree surfaces as a conflict for a human.
      Never resolved silently.
- [ ] Pending state survives a page reload — it lives in the ref, not in the page.

## Notes
This is where the `Trck-Op` trailer (#93zhqbd) and stacked replay (#9gxktnk) stop being
nice-to-have. Several pending commits at once are rare for a one-shot verb and routine for a
browser tab left open on a flaky network.
