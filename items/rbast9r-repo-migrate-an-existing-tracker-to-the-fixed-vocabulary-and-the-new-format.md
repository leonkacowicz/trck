# repo: migrate an existing tracker to the fixed vocabulary and the new format

## Summary
Existing trackers carry free-form statuses, priorities and resolutions, and no `format` key.
They need a one-shot migration, following the pattern `repo migrate-layout` already set: a verb
that is idempotent, previewable with `--dry-run`, and refuses to write when it cannot map
something unambiguously.

## Resolution: wontfix — hand-migrate instead
Only a handful of repos use trck, and the verb would not have saved much work on any of them.

The cases worth automating are the ones it *cannot* do: a status like `todo` or a priority like
`P0` has no unambiguous target, so the verb needs `--map old=new` — which is the mapping being
specified by hand anyway. What is left to automate is the case where the old name already equals
the new one, which needs no help.

Against that it is permanent surface: a second implementation in the Rust engine, conformance
fixtures for both, and a load-path guard. A prototype of that guard broke two existing tests
within minutes, one of them a legitimate renderer-robustness test asserting that an
out-of-vocabulary priority still sorts somewhere rather than crashing — behaviour the guard would
have made unreachable and the test meaningless.

`check` already *is* the migration guide. On an unmigrated tracker it names every problem and
every replacement:

```
warning: config: 'statuses' is no longer configurable and is being ignored
         (the vocabulary is fixed: backlog, ongoing, in-review, done)
warning: config: 'priorities' is no longer configurable and is being ignored
         (the priorities are fixed: urgent, high, medium, low, lowest)
error: #aaaaaaa unknown status 'todo'
error: #aaaaaaa bad priority 'P1' (expected one of: urgent, high, medium, low, lowest)
```

## Hand-migration recipe
Per repo: rewrite the values in `index.jsonl`, then replace `trck.json`, then `trck check`.

The **non-obvious part is which old status meant `in-review`** — and the old config recorded it,
in two places that are easy to miss:

- `statuses[].role`: `initial` -> `backlog`, `terminal` -> `done`. A status with no role was
  merely "active".
- `aliases`: the verb-to-status map. `aliases.review` names the waiting status, and it is the
  **only** evidence for `in-review` — no role was ever spelled for it. `aliases.start` likewise
  names the active one.

So for a config like
`{"statuses": [{"name":"todo","role":"initial"},{"name":"doing"},{"name":"reviewing"},{"name":"shipped","role":"terminal"}], "aliases": {"start":"doing","review":"reviewing","done":"shipped"}}`
the mapping is `todo->backlog`, `doing->ongoing`, `reviewing->in-review`, `shipped->done`.

Then reduce `trck.json` to just the format and update channel — every vocabulary key
(`statuses`, `aliases`, `priorities`, `default_priority`, `kinds`, `resolutions`) is ignored now,
and `check` warns until they are gone. Note `kind` survives as an ordinary custom field.

Reopen this if trck ever picks up users whose trackers predate the fixed vocabulary.
