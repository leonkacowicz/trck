# dates: due dates as a first-class field

## Summary
Every timestamp trck stores today is a *record of what happened*. A due date is the opposite: an
**intent** about the future. That makes it the only date feature here that needs a genuine schema
addition — and the only one that touches how work is ranked.

A half-working version already exists via free-form custom fields:
`trck set ID --field due=2026-08-15` stores it, `--sort field:due` orders by it, and
`--field due=2026-08-15` filters it — but only by exact match, with no validation that the value
is a date, and no view treats it as one. This epic decides whether to promote it and, if so, does.

The genuinely hard part is not storage: it's whether a deadline should influence `ready`/`next`,
which today rank by dependencies plus demand-propagated priority (#9bktptp). Adding time as a
third axis is a real change to the model, and a deadline is a *soft* signal that nothing enforces
— exactly the kind of thing priority already expresses. That question is split out as its own
decision so the field can land without it.

## Acceptance criteria
- [ ] `due` is validated wherever it's stored, and rendered as a date wherever it's shown.
- [ ] Overdue work is visible without asking for it.
- [ ] The ranking question is decided and recorded, either way.
- [ ] Children done.

## Notes
- Children: [[x6argpr]] (the field), [[3dtnmtv]] (the markers), [[h8yezpf]] (the ranking decision).
- Prior art in this repo for promoting an `extra` key to a canonical field: `pr` (#ecfntpa).
- If the decision is "don't promote it", closing this as `--resolution wontfix` and documenting
  the `--field due=…` recipe is a legitimate outcome.
