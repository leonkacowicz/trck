# Git-history timestamp backfill script — design

**Date:** 2026-06-12
**Status:** approved for implementation
**Sub-project:** 2 of 3

## Why

Sub-project 1 switched trck to stamp `created`/`started`/`closed` as full UTC ISO
timestamps, but left existing day-only values (`"2026-06-05"`) in place — the
engine reads them but never rewrites them. To make a release's own timestamp a
usable `--since` filter (sub-project 3), the historical values need real
time-of-day. That information is recoverable from git: the commit that set each
field is a faithful record of when it happened.

This sub-project is a standalone script that walks an index's git history and
rewrites each day-only `created`/`started`/`closed` into a UTC timestamp derived
from the **author date** of the commit that set the field to its current value.
It is a one-shot migration, runnable across any repo that uses trck.

## Scope

In scope:

- `scripts/backfill_timestamps.py` — stdlib-only, no engine import.
- Recover per-field timestamps from git history and rewrite the working-tree
  `index.jsonl` in place (with an opt-in `--dry-run`).
- Idempotent: values already in timestamp form are skipped.
- Unit + integration tests.

Out of scope:

- The `--since` changelog / release-notes verb (sub-project 3).
- Any change to the engine `./trck` (the engine already reads both forms).
- Backfilling anything other than the three date fields.
- Recovering sub-second precision or correcting clearly-wrong historical dates.

## Design

### Invocation

```
python3 scripts/backfill_timestamps.py [TRACKER_DIR] [--dry-run]
```

- `TRACKER_DIR` — positional, optional, defaults to `issues`. Must contain
  `index.jsonl`.
- `--dry-run` — do everything except write the file (still prints the report).

The script is committed in the trck repo and is intended to be copied/run against
other repos that use trck. It is **standard-library only** (`subprocess`, `json`,
`datetime`, `re`, `argparse`, `pathlib`, `sys`) and does **not** import the engine,
so it is portable and version-independent.

### Repo / path resolution

- Resolve the git root: `git -C <TRACKER_DIR> rev-parse --show-toplevel`.
- Compute `INDEX_REL` = path of `<TRACKER_DIR>/index.jsonl` relative to the git
  root (used verbatim in `git log`/`git show` pathspecs).
- Preconditions, each a clean error + nonzero exit:
  - `git` not found / `<TRACKER_DIR>` not inside a git repo.
  - `<TRACKER_DIR>/index.jsonl` does not exist in the working tree.

### Recovery algorithm

1. **List history**, oldest→newest, author date per commit:

   ```
   git -C <root> log --reverse --format=%H%x09%aI -- <INDEX_REL>
   ```

   Each line is `<full-sha>\t<author-date-iso8601>` (e.g.
   `a1b2c3…\t2026-06-06T12:34:56-03:00`).

2. **Walk commits.** Maintain `prev: dict[int, dict[str, str | None]]` — the last
   value seen for each issue's three fields. For each commit, oldest first:
   - Read the blob: `git -C <root> show <sha>:<INDEX_REL>`. If this fails
     (nonzero exit — e.g. the path didn't exist under that name at that commit),
     **skip the commit** (leave `prev` unchanged) and continue.
   - Parse each non-blank line as JSON into a row dict. For each row with an
     integer `id`, and for each field `f` in `("created", "started", "closed")`:
     - `cur = row.get(f)` (may be absent → treat as `None`).
     - `was = prev.get(id, {}).get(f)` (absent issue → all `None`).
     - If `cur is not None and cur != was`: this is a *transition into a new
       value* — record `recovered[(id, f)] = <commit author date>`, overwriting
       any earlier record.
     - Update `prev[id][f] = cur`.

   Because records are overwritten in commit order, `recovered[(id, f)]` ends up
   holding the author date of the **last** commit that set field `f` to the value
   it has at the end of history — i.e. when the field reached its current value.
   First appearance of an id is a transition from `None`, so `created` always
   resolves; a `closed` cleared by a reopen and later re-set resolves to the
   final close.

3. **Apply to the working tree.** Read the current `<TRACKER_DIR>/index.jsonl`.
   For each line (parsed as JSON), for each field `f` in the three:
   - Skip if absent or `None`.
   - Skip if not day-only — i.e. it does **not** match `^\d{4}-\d{2}-\d{2}$`
     (already a timestamp → idempotent skip).
   - Look up `recovered[(id, f)]`:
     - Found → set `row[f] = to_utc(author_iso)` and record the change for the
       report.
     - Not found → leave the value unchanged and emit a warning line (nothing is
       invented).

### `to_utc(author_iso) -> str`

```python
from datetime import datetime, timezone

def to_utc(author_iso: str) -> str:
    return datetime.fromisoformat(author_iso).astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
```

`datetime.fromisoformat` parses the offset-bearing git author date; `.astimezone(
timezone.utc)` normalizes to UTC; the format matches the engine's `now_utc()`
exactly (second-precision, `Z` suffix, no microseconds). Stdlib only.

### Write & format fidelity

Rewrite the file line-by-line: for each original line, parse → mutate only the
three date fields → re-serialize with `json.dumps(row, ensure_ascii=False)`, then
join with `"\n"` and a trailing newline. Key order is preserved (dict insertion
order is untouched), and the engine's canonical serialization is exactly
`json.dumps(row, ensure_ascii=False)` with default separators — so:

- Lines with no changed field round-trip **byte-identically**.
- Changed lines stay canonical (only the field value differs).

Therefore `trck check` passes after a run with no further `normalize`. Under
`--dry-run` the file is not written.

### Reporting

Always print a per-issue, per-field report of `#NNN <field>: <old> -> <new>` for
every change (whether writing or dry-running), a trailing summary count, and a
distinct `WARNING: #NNN <field> day-only but no history found` line for any
unresolved field. Under `--dry-run`, prefix the summary with a clear "dry-run, no
changes written" notice.

## Components / boundaries

- **`resolve_index(tracker_dir) -> (root, index_rel, index_path)`** — git/path
  resolution and preconditions. Depends on `git`.
- **`recover_times(root, index_rel) -> dict[(id, field), author_iso]`** — the
  history walk and transition reducer. Depends on `git log`/`git show`; pure given
  their output.
- **`to_utc(author_iso) -> str`** — pure timestamp conversion.
- **`backfill(index_path, recovered, dry_run) -> list[Change]`** — read, mutate
  day-only fields, optionally write, return the change list (also surfaces
  warnings). Pure except the optional write.
- **`main(argv)`** — argparse, wires the above, prints the report, sets exit code.

Each unit is independently testable; the only impure dependencies are `git`
(behind `recover_times`/`resolve_index`) and the single file write (behind
`backfill`).

## Error handling

- Missing `git`, dir not in a repo, or no `index.jsonl` → clear stderr message,
  nonzero exit, no write.
- A commit whose blob can't be read → skipped, not fatal.
- A day-only field with no recovered time → left as-is, warned, not fatal.
- Malformed JSON in the working-tree index → fail loudly (nonzero exit) rather
  than risk writing a corrupted file.

## Testing

Tests live in `tests/` and run under the existing
`python3 -m unittest discover -s tests` suite. They import the script the same way
`tests/helpers.py` imports the engine (a `SourceFileLoader` on
`scripts/backfill_timestamps.py`) for unit access, and shell out to it / to `git`
for integration.

- **`to_utc`**: `"2026-06-06T12:34:56-03:00"` → `"2026-06-06T15:34:56Z"`; an
  already-UTC `+00:00` input round-trips; offset crossing a day boundary
  normalizes correctly.
- **`recover_times` reducer**: drive it over a synthetic sequence of parsed index
  snapshots (monkeypatching the git-blob reader, or factoring the reducer to take
  an iterable of `(author_iso, rows)`), asserting: first appearance sets
  `created`; a later `closed` set wins over an earlier one (reopen→reclose); a
  field cleared to `None` does not overwrite the recovered time.
- **Day-only guard**: `"2026-06-05"` is rewritten; `"2026-06-05T00:00:00Z"` is
  skipped (idempotency).
- **Integration**: create a throwaway git repo in a `TemporaryDirectory`, write
  and commit an `index.jsonl` across several commits with controlled
  `GIT_AUTHOR_DATE` env values (one issue created, started, closed; one
  reopened→reclosed), run the script (subprocess), and assert the rewritten index
  has the expected UTC timestamps. Run it a second time and assert the file is
  unchanged (idempotent). Also assert `--dry-run` makes no change.
  - The git invocations set `GIT_AUTHOR_DATE`/`GIT_COMMITTER_DATE`,
    `GIT_AUTHOR_NAME`/`EMAIL` (and committer equivalents) explicitly so the test
    is deterministic and independent of global git config.

## Acceptance criteria

- [ ] `scripts/backfill_timestamps.py` exists, stdlib-only, does not import the engine.
- [ ] Runs as `python3 scripts/backfill_timestamps.py [TRACKER_DIR] [--dry-run]`, `TRACKER_DIR` defaulting to `issues`.
- [ ] Recovers each day-only `created`/`started`/`closed` from the author date of the last commit that set the field to its current value.
- [ ] Rewrites only day-only values; already-timestamped values are skipped (idempotent).
- [ ] Unchanged lines remain byte-identical; `trck check` passes after a run with no extra `normalize`.
- [ ] `--dry-run` prints the report but writes nothing.
- [ ] Clean errors (nonzero exit, no write) for: no git, dir not in a repo, missing `index.jsonl`, malformed working-tree JSON.
- [ ] Unresolved day-only fields are left untouched and warned about, not invented.
- [ ] Unit tests (`to_utc`, reducer, day-only guard) and a git-backed integration test (including reopen→reclose and idempotency) pass in the suite.
