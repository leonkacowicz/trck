# Timestamp data model — design

**Date:** 2026-06-12
**Status:** approved for implementation
**Sub-project:** 1 of 3

## Why

We want to facilitate cutting releases: answer "what issues were completed since
the last release?" using a release's own timestamp as a `--since` filter. The
prerequisite is that issue timestamps be precise enough to order events *within*
a day and to compare directly against a commit/release time. trck currently
stamps `created` / `started` / `closed` with a day-only date
(`date.today().isoformat()` → `"2026-06-05"`), which loses time-of-day and makes
"since this release at 14:30 UTC" impossible.

This sub-project converts the engine's stamping and reading to full UTC ISO
timestamps. It does **not** migrate existing data (sub-project 2, a standalone
git-history backfill script, does that) and does **not** add the release-notes
verb (sub-project 3). Those are designed separately after this lands.

## Scope

In scope:

- New `now_utc()` helper producing a second-precision, `Z`-suffixed UTC timestamp.
- The three stamp sites (`created`, `started`, `closed`) emit timestamps going forward.
- The engine reads **both** legacy date-only and new timestamp forms without error.
- Human-facing renders display a bare `YYYY-MM-DD` date slice for readability.
- Tests covering stamping, mixed-form reading/sorting, and display.

Out of scope (later sub-projects):

- Backfilling historical date-only values from git history (sub-project 2).
- The `--since` changelog / release-notes verb (sub-project 3).
- Any `--json` output work (tracked by the in-flight JSON-output epic, #24).

## Design

### Timestamp format

Second-precision UTC with a `Z` suffix: `2026-06-12T14:23:01Z`. No microseconds
(keeps `index.jsonl` lines short and the value easy to copy-paste as a `--since`
argument later). This is the canonical "now" form the engine writes.

Implementation: replace the `today()` helper (`trck:38`) with

```python
from datetime import datetime, timezone

def now_utc() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
```

`strftime` with an explicit `Z` literal avoids the `+00:00` that
`isoformat()` produces and drops microseconds in one step. The `date` import at
`trck:20` is replaced by `datetime, timezone`.

### Stamp sites

Three call sites swap `today()` → `now_utc()`; no other logic changes:

- `created=today()` on new issue creation (`trck:1274`).
- `started` on the initial→non-initial transition (`trck:1237`).
- `closed` on the transition into a terminal status (`trck:1240`).

The role-driven transition logic in `move_file` (`trck:1222–1242`) is otherwise
untouched — only the value being stamped changes.

### Read-both compatibility

No engine-side migration. Existing `"2026-06-05"` values remain valid and are
read verbatim. This works without special parsing because **both forms are
zero-padded ISO 8601 and compare correctly as plain strings**:

- `"2026-06-05" < "2026-06-05T14:23:01Z"` — a legacy date sorts as that day's
  start-of-day, before any timestamp on the same day. Correct.
- `"2026-06-05" < "2026-06-06"` and timestamp-vs-timestamp both order naturally.

Consequences:

- The `list` sort key (`created`, `trck:1497`) keeps using the raw string; mixed
  forms still sort correctly. No change needed beyond confirming with a test.
- `from_dict` already accepts `created`/`started`/`closed` as any string
  (`trck:325`); no validation change. We deliberately do **not** add a
  format check that would reject legacy date-only values.
- `check` and `normalize` accept both forms and never rewrite a date-only value
  into a timestamp. `normalize`'s job is canonical *slim* serialization, not
  time backfill; rewriting `"2026-06-05"` → `"2026-06-05T00:00:00Z"` would invent
  a time-of-day and is explicitly the backfill script's responsibility (which
  recovers the *real* time from git), not `normalize`'s.

### Display

Human-facing renders show only the date slice (first 10 chars) so output looks
unchanged and stays readable, while `index.jsonl` carries the full timestamp:

- `show` / `SUMMARY.md` closed annotation (`trck:1101–1102`):
  `(closed 2026-06-12)`, not `(closed 2026-06-12T14:23:01Z)`.
- Any other place that prints a `created`/`started`/`closed` value to a human.

A tiny helper keeps this consistent and legacy-safe (a date-only value's first 10
chars are already the date):

```python
def date_slice(ts: str | None) -> str:
    return ts[:10] if ts else ""
```

The full value is never altered on disk — only the rendered string is sliced.

## Components / boundaries

- **`now_utc()`** — sole producer of new timestamps. Input: none. Output: canonical
  timestamp string. Depends on `datetime`/`timezone`.
- **`date_slice()`** — sole consumer for human display. Input: a stored value (date
  or timestamp, or `None`). Output: a bare `YYYY-MM-DD` (or `""`). Pure.
- **Stamp sites** depend only on `now_utc()`; **render sites** depend only on
  `date_slice()`. Comparison/sort paths depend on neither — they use the raw
  string, relying on the zero-padded-ISO ordering invariant.

## Error handling

No new error paths. Reading remains lenient (any string accepted, as today). We
intentionally avoid adding timestamp-format validation so that legacy data and
partially-backfilled trackers never trip `check`.

## Testing

- **Stamp format:** creating an issue / starting / closing writes a value matching
  `^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}Z$`. (Stub `now_utc` to a fixed value so
  the assertion is deterministic — do not assert on real wall-clock time.)
- **Read both:** an index containing a legacy date-only `created` and a
  timestamped `closed` loads, passes `check`, and survives `normalize` with both
  values byte-identical (normalize must not expand the date-only one).
- **Mixed-form sort:** `list --sort created` orders a legacy-date issue and a
  same-day timestamped issue correctly (date sorts first).
- **Display:** `show` / `SUMMARY` of a timestamped closed issue renders
  `(closed YYYY-MM-DD)` with no `T…Z` tail; a legacy date-only issue renders the
  same way.
- Follows the existing TDD pattern; no test touches the real `./trck`
  (only `init`/`update` use `SELF_PATH`, neither involved here).

## Acceptance criteria

- [ ] `now_utc()` replaces `today()` and returns a `Z`-suffixed second-precision UTC timestamp.
- [ ] `created`, `started`, `closed` are stamped as timestamps on new writes.
- [ ] Legacy date-only values still load, `check` clean, and pass through `normalize` unchanged.
- [ ] `list` sort over mixed date/timestamp values orders correctly.
- [ ] `show` and `SUMMARY.md` display a bare `YYYY-MM-DD` date slice.
- [ ] Tests cover all of the above; full suite passes.
