# Custom fields (free-form key→value) — design

**Date:** 2026-06-11
**Status:** approved, pre-implementation

## Problem

Projects want per-issue metadata that trck doesn't model in its core vocabulary —
`assignee`, `reporter`, `component`, and the like. These mean different things in
different projects, so shipping them as built-ins would bloat the core mental model.
We want a generic way to attach arbitrary key→value pairs to an issue and to
**filter** and **sort** on them, without touching `trck.json` or the issue body.

## Key insight: the plumbing already exists

The `Issue` dataclass already carries an `extra: dict`. `from_dict` routes every
non-canonical key from `index.jsonl` into `extra`, and `to_canonical` writes those
keys back out — sorted, verbatim, after the known fields. `cmd_show` already prints
them. So this feature is **not** a storage change; it is a *user-facing surface* over
machinery that already round-trips. Existing trackers are forward-compatible, and a
future "declared schema" layer can be added with no migration.

## Decisions

- **Field model:** pure free-form. Any well-formed key, any **string** value. No
  declaration in `trck.json`. (Future nice-to-have: optional per-field schema —
  type, allowed values, required-ness — added later without migration.)
- **Value type:** always a string. Sorting is therefore lexicographic for now;
  typed sort is a future concern tied to declared schemas.
- **Set syntax:** extend the existing `set` verb (one home for all mutations).
- **Filter/sort syntax:** generic `--field`/`--sort field:NAME` on `list`, mirroring
  the set vocabulary so there is one "field" word across the CLI.
- **Display:** `show` always lists extras (unchanged); `list` stays clean by default
  and only shows a custom column when explicitly asked.

## Surface

### Writing — `set`

```
trck set NNN --field key=value      # set / overwrite (repeatable)
trck set NNN --unset key            # remove (repeatable)
trck set NNN --field key=           # empty value == remove (alias for --unset)
```

- `--field` and `--unset` are `action="append"`, so multiple may be given in one call
  and combined with the existing `--priority` / `--kind` / … flags.
- Applied in order; later `--field`/`--unset` for the same key wins.

**Guards (fail-loud, via `die`):**

- **Key shape:** must match `^[a-z][a-z0-9_-]*$`. Rejects whitespace, `=`, uppercase,
  empty keys — keeps `index.jsonl` tidy and filter values predictable.
- **Reserved keys:** a key equal to any canonical field name (`CANON_KEYS`:
  id, slug, title, kind, status, priority, points, parent, labels, depends_on, spec,
  created, started, closed, resolution) is rejected with a message pointing at the
  proper flag/verb (e.g. "`status` is a built-in; use `trck mv`").

### Reading — `list`

```
trck list --field key=value         # filter, exact string match (repeatable, AND-ed)
trck list --sort field:NAME         # sort by that field's value
trck list --show-field NAME         # add a trailing column (repeatable, opt-in)
```

- **Filter:** `--field k=v` keeps rows where `extra.get(k) == v`. Repeatable;
  multiple are AND-ed; composes with `--status`/`--kind`/`--priority`/`--label`/etc.
- **Sort:** `--sort field:NAME` extends the existing `--sort` (`id`/`priority`/
  `points`/`created`). Key is `(0/1 present-flag, value, id)` so rows **missing** the
  field sort **last**, ties broken by id. Lexicographic on the string value.
- **Show column:** `--show-field NAME` appends a dimmed trailing `NAME=value` segment
  per row (nothing when the row lacks the field). `list` is otherwise unchanged.

### Display — `show`

`trck show NNN` already prints extras after the canonical fields. No change.

## Validation — `check`

`validate`/`check` gains a light rule: every key in `extra` must match the key shape
and every value must be a string. This catches hand-edits to `index.jsonl` and keeps
the free-form space well-formed. Fail-loud, consistent with the rest of `check`.

## Implementation notes (engine bands)

- `cmd_set`: parse `--field`/`--unset`; validate key shape + reserved collision;
  mutate `row.extra`; `finalize` as today. (`set` already moves files / rewrites the
  title; custom fields never affect the filename.)
- `cmd_list` / `keep` predicate: add the `--field` AND filter.
- `cmd_list` `sort_keys`: special-case a `field:` prefix on `args.sort`.
- `print_rows`: accept an optional `show_fields` list; append dimmed `name=value`
  segments after labels.
- `validate`: add the extra-keys/values rule.
- argparse: add `--field`/`--unset` to `set`; `--field`/`--show-field` to `list`,
  and document the `field:NAME` sort value in `--sort` help.
- Help text: update the embedded usage (`cmd_help`) and `main` epilog examples.

## Out of scope (future)

- Declared schemas in `trck.json` (per-field type, allowed values, required-ness).
- Typed sort/compare (numeric, date).
- Per-field validation beyond "is a string".
- Filtering by absence/presence beyond the `--field k=` empty-value clear on `set`.

## Tests (TDD)

- `set --field k=v` adds to `extra`; round-trips slim + sorted in `index.jsonl`.
- `set --field k=v` overwrites an existing value.
- `set --unset k` and `set --field k=` (empty) both remove.
- `set --field` with a reserved/canonical key → error.
- `set --field` with a malformed key (uppercase, space, leading digit) → error.
- `list --field k=v` filters; two `--field`s are AND-ed; composes with `--status`.
- `list --sort field:NAME` orders correctly; missing-value rows sort last.
- `list --show-field NAME` adds the column; absent value shows blank.
- `show` displays extras (confirm).
- `check` passes with custom fields present; fails on a non-string extra value.
