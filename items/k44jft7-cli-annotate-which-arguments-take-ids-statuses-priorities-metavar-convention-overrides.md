# cli: annotate which arguments take ids/statuses/priorities (metavar convention + overrides)

## Summary
The completion callback can read the parser tree for verbs, flags and `choices`, but nothing in
`build_parser()` records *what kind of value* a free-form argument accepts: `mv`'s `status`
positional, `set --priority`, `dep --add` and `new --parent` are all plain strings as far as
argparse is concerned.

Establish a **metavar convention** — `ID`, `STATUS`, `PRIORITY`, `KIND`, `RESOLUTION`, `LABEL` —
as the primary signal, since it doubles as a `--help` improvement (`trck mv ID STATUS` reads
better than today's `trck mv id status`). Back it with a small explicit override table for the
arguments the convention can't express.

## Acceptance criteria
- [ ] Every argument taking a tracker value carries a metavar naming the value kind.
- [ ] An override table covers what the convention can't: `list --status` (comma list with a
      leading `!`), `set --field KEY=VALUE`, `diff REV`, and the path-valued flags.
- [ ] One helper answers "what kind of value goes here?" for a (verb, action) pair — the
      callback never re-derives it.
- [ ] `--help` wording is unchanged; only the metavar placeholders improve.
- [ ] Tests assert the kind resolved for a representative argument of each kind, so a new flag
      added without an annotation is visible.

## Notes
- Deliberately *not* a custom `add_argument` kwarg — argparse rejects unknown kwargs. Setting an
  attribute on the returned Action (`mv.add_argument(...).completes = "status"`) is the fallback
  if the metavar convention proves too lossy: it keeps the annotation next to the definition, at
  the cost of noise in `cli.py`.
- Watch the collisions when writing the table: `--parent` and `--depends` take ids despite being
  named for the relation, and `list --priority` filters rather than sets.
- Blocks [[qhf5fa2]]. Part of [[9echsrh]].
