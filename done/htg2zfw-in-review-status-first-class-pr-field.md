# in-review status + first-class pr field

## Summary
Give work that is implemented-but-not-merged an honest place in the vocabulary, and
record which pull request it is waiting on.

See the spec:
[`docs/specs/2026-07-28-in-review-status-and-pr-field-design.md`](../../docs/specs/2026-07-28-in-review-status-and-pr-field-design.md)

Three threads, decomposed into the children below:
1. `in-review` joins the default vocabulary, plus a generic `"actionable": false`
   status flag so `ready`/`next` skip waiting states.
2. `pr` becomes a built-in field (validated URL, set via `new`/`set`/`mv`, rendered as
   a link in `SUMMARY.md` and `trck-html`).
3. `trck review ID [URL]` — one verb for the moment a PR appears.

## Acceptance criteria
- [ ] All children done
- [ ] `trck check` clean, full suite green, `build.py --check` in sync
- [ ] A tracker with no PRs and no in-review issues serializes identically to before

## Notes
Children are sequenced with dependencies, not by nesting order.
