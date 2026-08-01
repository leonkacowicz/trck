# dates: time-in-status and per-status cycle time reporting

## Summary
The ambitious end of the date work: answer "where does time actually go" per status, not just
overall. How long does work wait in `backlog` before someone starts it, how long does a PR sit in
`in-review`, and how has that changed over the last few months.

None of it is derivable from the three stored timestamps — `started` and `closed` bracket the whole
active span and say nothing about the statuses in between. It needs the transition history from
[[gybeetp]], which is why this sits behind that decision rather than under the
no-schema-change epic [[922fmtw]].

Likely surface, once the data exists:

- Per-issue: time spent in each status, on `show`.
- Aggregate: median/p90 time in each status across a set of issues, respecting `list`'s filters so
  you can scope by label, epic or date range.
- Throughput: points or issues reaching a terminal status per week — the natural companion to
  `changelog`, and the obvious data source for a chart in the browser tool.

Statuses are per-repo configuration, so every output here must be driven by `trck.json`'s
vocabulary, never by hard-coded names.

## Acceptance criteria
- [ ] Per-status durations are computed from the transition history chosen in [[gybeetp]].
- [ ] The reporting surface (a verb, flags on `list`, or both) is decided before implementation —
      split into sub-issues at that point rather than building it as one lump.
- [ ] Output is vocabulary-driven: no status name hard-coded in the engine.
- [ ] Aggregates state their sample size, and behave sanely on a tracker with almost no history.

## Notes
- Needs [[gybeetp]]. Decompose this into real sub-tasks once that decision fixes the data model —
  the 5 points here are a placeholder for a subtree, not a single sitting.
- Reopened issues make "time in status" ambiguous (sum the visits, or take the last?). Decide when
  scoping.
- Related: [[922fmtw]] covers the durations that need no history; keep the two from overlapping.
