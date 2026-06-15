# Harden Issue.from_dict with a structural/type contract

## Summary
Follow-up to #022. The `Issue` dataclass introduced in #022 defaulted its
identity/state fields to `None` so a malformed `index.jsonl` row would still
construct, deferring all checking to `validate`. Make the parse layer own the
*structural* contract instead: the six identity/state fields (id, slug, title,
kind, status, priority) become required and non-optional, and `from_dict` fails
loud on a missing required field or a wrong-typed value, so a bad row is never
turned into a half-built `Issue` that crashes somewhere downstream.

## Acceptance criteria
- [x] id/slug/title/kind/status/priority are required, non-optional fields.
- [x] `from_dict` raises `ValueError` on a missing required field or wrong-typed
      value; `load_index` reports it as an `index.jsonl line N: ...` failure.
- [x] `validate` drops the now-redundant structural checks (points int-ness,
      labels list-of-strings) and keeps only value/config/graph consistency.
- [x] Behaviour and on-disk format unchanged; full suite green.

## Notes
- Split structural (parse-time, fail-loud) from semantic (validate-time,
  collected) checking — parse guarantees types so `validate` trusts them.
- `cmd_new` parses `points` explicitly; `cmd_version` uses
  `resolve_tracker_dir` rather than building a full `Ctx`.
- Done in commit `a19562d`. 147 tests pass; `check` clean.
