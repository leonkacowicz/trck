# dates: duration helpers (age, lead time, cycle time) and show them in trck show

## Summary
Add the duration primitives the rest of the epic builds on, and make `trck show` the first
consumer so they're usable and tested end to end.

Three derived values, all from the timestamps already stored:

- **age** — `created` → now, for an issue that isn't terminal. "How long has this been open."
- **lead time** — `created` → `closed`. The whole wall-clock life of the issue.
- **cycle time** — `started` → `closed`. How long it took once someone actually picked it up.

Plus a human formatter (`3d`, `2w`, `4mo`) so callers don't each invent one, and a parse-back of
the stored timestamps into `datetime` — the engine currently only ever *writes* them with
`now_utc()` and slices the first 10 characters to display (`constants.py::date_slice`).

`trck show` gains the durations next to the timestamps it already prints: an age line for open
issues, lead/cycle time for closed ones.

## Acceptance criteria
- [ ] A parse helper turns a stored timestamp into a `datetime` (and tolerates the bare-date form
      a hand-migrated tracker might carry).
- [ ] `age`, `lead_time`, `cycle_time` return a `timedelta` or `None` when their inputs are absent.
- [ ] A formatter renders a duration compactly and consistently (agree the units and rounding).
- [ ] "Now" is a parameter, not an ambient `now_utc()` call, so tests are deterministic.
- [ ] `trck show` displays age for open issues and lead/cycle time for terminal ones.
- [ ] Tests cover each helper, missing inputs, and the `show` rendering.

## Notes
- Timestamps: `index.py:79-81`; written in `cmd_mutate.py:36` and `templates.py:193-199`.
- An issue reopened out of a terminal status has `closed` cleared (`templates.py:199`) but keeps
  its original `started` — decide whether cycle time should reflect that or reset.
