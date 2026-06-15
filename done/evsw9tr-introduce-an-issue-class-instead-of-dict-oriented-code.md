# Introduce an Issue class instead of dict-oriented code

## Summary
The engine currently represents an issue as a bare `dict` (the parsed `index.jsonl`
record) threaded through the whole codebase. Fields are accessed by string keys
(`rec["status"]`, `rec.get("parent")`), defaults are re-applied ad hoc at each call
site, and there's no single place that defines what an issue *is*. Introduce a proper
`Issue` definition (e.g. a `@dataclass`) so the shape, defaults, and (de)serialization
live in one place, replacing the scattered dict access.

## Acceptance criteria
- [x] A single `Issue` type defines all fields, types, and defaults.
- [x] Centralized `from_dict` / `to_dict` (or equivalent) for index.jsonl round-trips, preserving the existing canonical slim form (no spurious diffs in `index.jsonl`).
- [x] Command handlers and validation operate on `Issue` instances rather than raw dicts.
- [x] Stays standard-library only (`dataclasses` is fine).
- [x] All existing tests pass; behaviour and on-disk format are unchanged.

## Notes
- Engine is the single file `./trck`; keep the band organization intact.
- Watch the round-trip: `trck normalize` output and `check` must be byte-stable so
  existing `index.jsonl` files don't churn.
- Pure refactor — no user-visible behaviour change intended.
- Done: added an `@dataclass Issue` (typed known fields + an `extra` dict for
  custom/forward-compatible keys) owning `from_dict`/`to_canonical`/`to_dict`.
  `load_index`/`save_index` now speak `Issue`; every handler, `validate`, summary
  and tree rendering use attribute access. **Pure** — no mapping shim; the
  dict-touching tests were rewritten to construct/read `Issue`. A new
  `tests/test_issue.py` covers the model (defaults, extra, milestone migration,
  canonical round-trip). `normalize` on the live tracker is byte-identical and
  `check` passes; full suite green (141 tests).
- The test loader (`tests/helpers.py`) now registers the module in `sys.modules`
  before exec: on Python 3.12+ `@dataclass` under `from __future__ import
  annotations` resolves field annotations via `sys.modules[cls.__module__]`.
