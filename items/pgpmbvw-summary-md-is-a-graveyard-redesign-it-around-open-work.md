# SUMMARY.md is a graveyard: redesign it around open work

## Summary
`SUMMARY.md` is still in its first format and has stopped being read. It is 254 non-blank lines, of
which the open backlog is 27. The rest is finished work: 182 of 232 issues are done, and 13 of 19
epics are 100% complete and still print their full child checklist — `#sp2rwzx` alone is 40 lines of
closed sub-tasks. `list` learned to hide settled work; the summary never did.

Four separate problems, one per child:

1. **It is mostly a graveyard.** Completed epics print in full, and the Done section is ordered by
   *priority*, so the history leads with urgent things closed months ago rather than what shipped
   last. → [[summary-hide-settled-work-and-collapse-completed-epics]]
2. **Every row carries ~70 characters of link URL restating its own title.** Rendered on GitHub that
   is invisible; in an editor, which is where it is actually read, it is most of the line. Median
   line width is 162 characters. →
   [[summary-reference-style-links-so-a-row-reads-in-raw-markdown]]
3. **It never answers the question you would open it for.** Nothing says what is in flight or what to
   pick up next, though `ready`/`next` compute exactly that. →
   [[summary-lead-with-in-flight-and-next-up]]
4. **Two organizing axes at once, with holes.** "Hierarchies" holds epics and their children; the
   status sections hold everything else — so *where* an issue appears depends on whether it happens
   to have a parent, and some rows can appear nowhere at all. →
   [[summary-partition-open-work-every-non-terminal-row-exactly-once]]

A worked sketch against this repo's live tracker came out at **86 content lines** with all four
applied, and a **median width of 92** once the links are reference-style. The shape it landed on:

    # Issues

    **46 backlog · 4 in-progress · 0 in-review · 182 done** — 232 issues, 78% complete by points

    ## In flight
    ## Next up
    ## Open work        (open epics, open children only, `_N done_` footer; then Unfiled)
    <details>Done — newest first</details>

## Acceptance criteria
- [ ] All four children done, and `SUMMARY.md` regenerated in the same change.
- [ ] The conformance goldens that assert `SUMMARY.md` are updated, and at least one new fixture
      covers the pruning rule (a fully-done epic, an open epic with done children).
- [ ] `docs/` or the README says what the file is for in one line, so the next reader knows whether
      to scan it or open `trck list`.

## Notes
- Generator is `src/summary.rs` (267 lines). Every mutating verb regenerates the file, so any change
  here rewrites it on the next write regardless of when it lands.
- **The framing question underneath all four:** is `SUMMARY.md` a *report* (an inventory, grouped by
  status, stable to read) or a *browse view* (what `trck list` shows, rendered for GitHub)? The
  children are all compatible with either, deliberately — but
  [[summary-html-follow-list-s-topological-sibling-order-or-state-why-not]] (`#bbek25a`) is the same
  question asked about ordering, and the two should be answered together.
- [[summary-include-foreign-unknown-statuses-in-the-counts-table]] (`#m3z2ywb`) is about the counts
  table this epic rewrites. Fold it in or close it as superseded when the header lands.
- The sketch is not committed anywhere; it was generated from `issues/index.jsonl` to size the
  problem, not to be the implementation.
