# index/set: due as a first-class validated date field

## Summary
Promote `due` from a free-form custom field to a canonical one, following the path `pr` took in
#ecfntpa: a slot on `Issue`, an entry in `CANON_KEYS`, type/shape enforcement in
`Issue.from_dict`, a `--due` flag on `new`/`set` (with `--due none` to clear), and a `check` rule.

Unlike `created`/`started`/`closed`, this one is **user-supplied**, so it needs real validation —
`from_dict` currently only asserts that the timestamp fields are strings (`index.py:145`), which is
fine for values the engine itself wrote and not fine for a value typed at the command line. A
`due` must be a well-formed calendar date, and `check` must reject a malformed one that arrives
via a hand-edit or a bad merge.

Store a bare `YYYY-MM-DD`, not a timestamp: a deadline is a day, not an instant, and the bare form
still compares correctly against the stored timestamps lexicographically.

## Acceptance criteria
- [ ] `due` is a canonical `Issue` field, in `CANON_KEYS`, slim-serialised (omitted when unset).
- [ ] `trck new --due` and `trck set --due DATE|none` write and clear it.
- [ ] A malformed date is rejected at the CLI with a clear message, and by `trck check` when it
      arrives some other way.
- [ ] `trck show` displays it; `--sort due` orders by it with missing values last.
- [ ] An existing tracker with `due` in `extra` migrates cleanly (or the collision is handled
      explicitly and documented).
- [ ] `trck repo normalize` round-trips it; tests cover the field, the flags, and validation.

## Notes
- Touch points: `index.py:14` (`CANON_KEYS`), `index.py:79-82`, `index.py:145`, `cmd_mutate.py`,
  `cmd_query.py:127` (sort keys), `diff.py` (decide whether a `due` change is a reportable delta —
  it is: unlike the evidence-only timestamps at `diff.py:17`, a moved deadline is a real edit).
- Watch the reserved-key guard from #45mc92r: once `due` is canonical, `--field due=…` must stop
  being accepted.
