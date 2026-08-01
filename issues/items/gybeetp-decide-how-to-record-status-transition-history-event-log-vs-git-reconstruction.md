# decide how to record status-transition history: event log vs. git reconstruction

## Summary
Only the *latest* `started` and `closed` are kept (`templates.py:193-199`), and a reopen clears
`closed` outright. So the tracker can answer "when did this finish" but not "how long did it sit in
review", "how many times was this reopened", or "what did the backlog look like in May". This is
the fork in the road that gates every ambitious date feature; decide it before building on either
side.

**Option A — an event log.** Append transitions to a companion file (`events.jsonl`) or an array
on the row. Explicit, self-contained, works without git, survives a tracker copied out of its repo.
Costs: a schema change, a growing file, new merge semantics (the row-wise driver from #ey2aruc
merges by id — an append-only log needs its own rules), and `check` has to validate that the log
agrees with the row's current status.

**Option B — reconstruct from git.** `index.jsonl` is committed, so its git history *already* is
the event log: every commit is a full snapshot, and the transitions are the diffs between them.
The machinery exists — `diff.py` compares two snapshots field by field and `gitsrc.py` loads an
index at an arbitrary ref, deliberately isolated behind a VCS-agnostic seam (#q9cq65c). Zero schema
change, zero merge risk, nothing to keep consistent, and it retroactively covers all existing
history. Costs: git-only (a tracker outside a repo has no history), slow on a long history
(one `git show` per commit), and it conflates a transition with the commit that recorded it —
batched tracker commits blur timing to commit granularity.

Not mutually exclusive: B could ship first as the cheap answer, with A added later only if the
granularity turns out to matter.

## Acceptance criteria
- [ ] The decision is made and recorded here, with the reasoning and the rejected option.
- [ ] The consequences for `trck check`, the merge drivers, and tracker size are stated explicitly.
- [ ] Follow-ups ([[xr994r6]], [[ut9bqm4]]) are re-scoped against the chosen approach.

## Notes
- Existing seams that make B cheap: `gitsrc.py`, `diff.py` (`TIMESTAMP_FIELDS` at `diff.py:17`
  already treats timestamps as evidence of a change rather than a change), `q9cq65c`'s source seam.
- Initial lean: B. Reconstruction costs nothing, can't drift, and covers history that already
  exists — and B's weakness (commit granularity) is mostly a documentation problem.
- The engine is standard-library only; B shells out to git, which `diff.py` already establishes as
  acceptable behind the seam.
