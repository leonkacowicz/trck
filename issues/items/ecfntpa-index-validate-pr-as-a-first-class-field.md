# index/validate: pr as a first-class field

## Summary
Add `pr` to the `Issue` dataclass and `CANON_KEYS`, directly after `spec` — both point
at an external document. `FIELD_DEFAULTS["pr"] = None`, so a row without a PR
serializes exactly as before and adopting this causes no index churn.

Validate the value as an absolute http(s) URL (`PR_URL_RE`) through a `check_pr`
predicate shared by the command handlers and by `validate` (which catches hand-edits).
Forge-agnostic — trck does not know what GitHub is.

## Acceptance criteria
- [ ] `pr` round-trips through `index.jsonl`; the key is absent when unset
- [ ] `from_dict` rejects a non-string `pr`
- [ ] `check` errors on a non-URL `pr` value
- [ ] `--field pr=…` is rejected as a built-in (existing reserved-key message)
- [ ] `show` prints `pr` when set

## Notes
Reserving `pr` is a (tiny) breaking change for a tracker already using a custom field of
that name — the value still round-trips, now as a canonical field.
