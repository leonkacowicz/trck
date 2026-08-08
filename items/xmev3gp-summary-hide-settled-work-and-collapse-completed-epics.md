# summary: hide settled work and collapse completed epics

## Summary
182 of 232 issues are done, and the summary prints nearly all of them at full weight. 13 of 19 epics
are 100% complete and still list every child; `#sp2rwzx` alone is 40 lines of closed sub-tasks. The
open backlog — the reason to open the file — is 27 lines out of 254.

Three changes, all in the generator:

- **A completed epic collapses to one line.** Its heading with the rollup, no checklist.
- **An open epic lists only its open children,** with a `_N done_` footer standing in for the rest, so
  the progress number still has something behind it.
- **Done moves into a `<details>` block, newest first.** It is currently ordered by *priority*, so the
  history section leads with urgent things closed months ago instead of what shipped last — which is
  the only question anyone asks of a done list.

Against this repo's live tracker the sketch came out at **86 content lines from 254**.

The pruning rule is [[summary-partition-open-work-every-non-terminal-row-exactly-once]], which lands
first — this issue is the rendering, that one is the rule.

## Acceptance criteria
- [ ] A terminal epic renders as a single line; a non-terminal epic renders only its non-terminal
      children plus a count of the rest.
- [ ] The Done section is ordered by `closed`, newest first, and is inside `<details>` with a count in
      the `<summary>`.
- [ ] Nothing is *dropped* — collapsed rows are still reachable through the epic's own file, and the
      counts add up to the index.
- [ ] Conformance fixtures: a fully-done epic, an open epic with a mix of done and open children, and
      a tracker with nothing done at all (the block must not render as an empty `<details>`).
- [ ] Existing `expected.SUMMARY.md` goldens updated in the same change.

## Notes
- `src/summary.rs`. Every mutating verb regenerates the file, so this rewrites `issues/SUMMARY.md`
  on the next write whenever it lands.
- `<details>` renders on GitHub and degrades to visible text in a plain markdown viewer, which is the
  right failure mode — worst case the reader sees what they see today.
