# list/ready: --show-age column and a staleness marker on rows

## Summary
Surface age where you actually triage: on `list` and `ready` rows. Two affordances, both in-place
rather than a new view or verb.

**`--show-age`** — an opt-in trailing column, exactly like `--show-field NAME` already works
(`cmd_query.py`, added by #wn45zbq). Shows age for open rows, lead time for terminal ones.

**A staleness marker** — a dim marker on rows that have been open unusually long, shown by
default so you notice it without asking. It sits alongside the existing dim `needs #NNN` /
`blocks #NNN` annotations and the `↑<priority>(#id)` demand marker, in the same shared row
renderer. The threshold should come from `trck.json`, not be baked in — the engine's vocabulary is
data-driven, and "stale" means something different per repo.

## Acceptance criteria
- [ ] `--show-age` adds a trailing duration column on `list` and `ready`.
- [ ] Rows past the staleness threshold carry a dim marker by default; a flag suppresses it.
- [ ] The threshold is read from `trck.json` with a sensible default, not hard-coded.
- [ ] Terminal rows are never marked stale.
- [ ] `SUMMARY.md` is unaffected.
- [ ] Tests pin the column, the marker at/around the threshold, and the interaction with the
      existing annotations.

## Notes
- Row rendering is shared — see the extracted renderer from #33frt7s; add here, not per-verb.
- Needs the helpers from [[hfbe4n2]].
- Open question: should staleness measure `created`→now, or "time since the last transition"?
  The latter is truer to "nothing is happening here" but isn't derivable for a row that never
  moved past its initial status. Probably `started`→now when started, else `created`→now.
