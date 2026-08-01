# dates: shared cutoff parser — accept relative forms (7d, 2w, yesterday) and resolve to absolute

## Summary
`parse_since` currently accepts only a bare date (`YYYY-MM-DD`) or a full UTC timestamp
(`YYYY-MM-DDTHH:MM:SSZ`), validated by `SINCE_RE` and returned unchanged. Every date-flavoured
flag we add will want the same input handling, so promote it into one parser that also accepts
relative forms and **resolves them to an absolute value at parse time**.

Resolving eagerly is the whole point: downstream code keeps comparing plain ISO strings, and the
command still prints the absolute cutoff it used, so a rerun of the echoed command reproduces the
same answer. "Now" enters only at argument parsing, never at rendering.

Forms to accept, on top of what already works:

- `<N>d`, `<N>w`, `<N>m`, `<N>y` — N days/weeks/months/years back from today (UTC).
- `yesterday`, `today`.

## Acceptance criteria
- [ ] One parser (in `constants.py` or `cmd_maint.py`) handles absolute dates, absolute
      timestamps, and the relative forms above, returning an absolute string.
- [ ] `changelog --since 7d` works and its header shows the resolved absolute cutoff, not `7d`.
- [ ] A malformed cutoff still dies with a message naming every accepted form.
- [ ] Tests cover each relative form, the absolute passthrough (byte-identical), and rejection.

## Notes
- Today: `constants.py::SINCE_RE`, `cmd_maint.py::parse_since` (~line 139), consumed by
  `cmd_changelog` and `select_shipped` (plain ISO `<` compare).
- Month/year arithmetic has no stdlib helper — `timedelta` covers d/w; m/y needs a small
  clamp-the-day-of-month routine. Consider dropping `m`/`y` if that's not worth the code.
- Wanted by [[v5wvabj]] (`list --since/--until`); that issue can ship with absolute-only
  parsing if this lands later, so the edge is a preference, not a dependency.
