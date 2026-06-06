# Introduce an Issue class instead of dict-oriented code

## Summary
The engine currently represents an issue as a bare `dict` (the parsed `index.jsonl`
record) threaded through the whole codebase. Fields are accessed by string keys
(`rec["status"]`, `rec.get("parent")`), defaults are re-applied ad hoc at each call
site, and there's no single place that defines what an issue *is*. Introduce a proper
`Issue` definition (e.g. a `@dataclass`) so the shape, defaults, and (de)serialization
live in one place, replacing the scattered dict access.

## Acceptance criteria
- [ ] A single `Issue` type defines all fields, types, and defaults.
- [ ] Centralized `from_dict` / `to_dict` (or equivalent) for index.jsonl round-trips, preserving the existing canonical slim form (no spurious diffs in `index.jsonl`).
- [ ] Command handlers and validation operate on `Issue` instances rather than raw dicts.
- [ ] Stays standard-library only (`dataclasses` is fine).
- [ ] All existing tests pass; behaviour and on-disk format are unchanged.

## Notes
- Engine is the single file `./trck`; keep the band organization intact.
- Watch the round-trip: `trck normalize` output and `check` must be byte-stable so
  existing `index.jsonl` files don't churn.
- Pure refactor — no user-visible behaviour change intended.
