# Strip null/empty fields from index.jsonl rows to reduce noise

## Summary
Each `index.jsonl` row currently serializes **every** `CANON_KEY`, including ones that
carry no information: `milestone: null`, `parent: null`, `resolution: null`,
`depends_on: []`, etc. (`save_index` builds `ordered` from `CANON_KEYS` with `None`/`[]`
defaults — `trck:207`). This makes each line wide and noisy and buries the few fields that
actually say something.

Strip fields that equal their default on write, so a row only carries the keys that say
something non-default. Readers already tolerate this — every field access goes through
`.get()` (`load_index` re-defaults `depends_on`, `validate`/`generate_summary`/`show` use
`r.get(...)`), so this is a write-side change with no reader changes required.

**Strip criterion is "equals the field's default", not "is empty/falsy".** For most
fields the default is `None` (or `[]` for `depends_on`), so the redundant case is the
empty one. But a field can have a non-empty default: `points` (#019) defaults to `1`, so
`points: 1` is the redundant line to strip — while `points: 0` is *real information*
("trivial task") and must be kept. A generic emptiness/falsiness test would get this
backwards (drop the `0`, keep the `1`), so the rule must compare against per-field
defaults.

## Acceptance criteria

### What gets stripped (write side — `save_index`)
- [ ] Introduce a per-field default map (e.g. `FIELD_DEFAULTS`) covering the CANON_KEYS
      that have one: `depends_on -> []`, the rest of the optional fields `-> None`
      (`milestone`, `parent`, `resolution`, `spec`, `started`, `closed`, ...). This is the
      single source of truth that `save_index`'s current inline
      `[] if k == "depends_on" else None` default already implies — promote it to a named
      map and reuse it.
- [ ] A field is omitted from the serialized object iff its value **equals that field's
      default**. Fields with no declared default (the always-present required ones —
      `id`, `slug`, `title`, `status`, `kind`, `priority`, `created`) are never stripped.
- [ ] Consequence to verify explicitly: `points: 1` is stripped (default), `points: 0`
      is kept (non-default — "trivial"), `points: 3` is kept. Numeric `0` / `False` are
      never collateral-stripped because the test is equality-to-default, not falsiness.
- [ ] CANON_KEYS ordering is preserved for the keys that *are* present; unknown/extra keys
      still follow in the existing stable sorted order.
- [ ] Output is idempotent: loading a stripped index and re-saving produces byte-identical
      lines (no field reappears, no reordering).

### Read side
- [ ] Confirm no code path indexes a stripped field with `r["key"]` where the key may now
      be absent. Audit the `r["parent"]`/`r["milestone"]`-style direct subscripts
      (`trck:303`, `:311`, `:417`, `:465`, `:486`) — each is already guarded by a prior
      `.get(...)` truthiness/`is not None` check, but verify rather than assume.
- [ ] `load_index` keeps re-defaulting `depends_on` to `[]` so downstream `for dep in
      r["depends_on"]` style access is safe.

### Validation / round-trip
- [ ] `check` passes on a freshly stripped index.
- [ ] Re-running `trck summary` / any verb that calls `save_index` over the existing repo
      issues produces the stripped form and `check` stays green.

### Tests (TDD)
- [ ] `save_index` omits fields equal to their default (`milestone: None`,
      `depends_on: []`, ...) and keeps non-default ones in CANON order.
- [ ] Non-empty default case: a leaf with `points: 1` (default) is stripped, `points: 0`
      and `points: 3` (non-default) are kept. (Can land with #019; until then, assert the
      equality-to-default behavior with whatever field exercises it.)
- [ ] Round-trip: `load_index(save_index(rows))` equals the logical rows, and a second
      `save_index` is byte-identical to the first.
- [ ] A row with a populated `milestone`/`parent`/`resolution`/`depends_on` still
      serializes those fields.

## Notes
- This is purely a serialization-format change; the in-memory row dicts are unchanged.
  Existing `index.jsonl` lines remain valid input (extra null keys load fine and simply
  get dropped on the next write).
- The first write after this lands will rewrite all 20-ish existing rows into the slimmer
  form — expect a one-time churn diff on `index.jsonl`. No migration step needed.
- Decision: strip by **equals-the-default test, not emptiness/falsiness**. The default is
  per-field — `None`/`[]` for most, but `1` for `points` (#019). So `points: 1` is the
  redundant line that gets stripped, while `points: 0` ("trivial") is kept as real signal.
  A naive `if not value` would invert this (drop the `0`, keep the `1`) — guard against
  that simplification.
- Touchpoint is small: `save_index` (`trck:203`). Keep the `depends_on` default in
  `load_index` as the safety net for that one field.
