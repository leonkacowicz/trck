# `in-review` status + first-class `pr` field — design

**Date:** 2026-07-28
**Status:** approved, pre-implementation

## Problem

Work that has been implemented but not yet merged has no honest place in the default
vocabulary. `ongoing` says "someone is typing"; `done` says "shipped". A pull request
sits between them: the work is finished, the author is not going to pick it back up,
and yet nothing downstream may proceed until it merges.

Two things are missing:

1. A **status** for that waiting state — and, crucially, one that `ready`/`next` do
   *not* propose as work to pick up. An issue awaiting review is in flight, not
   available.
2. A place to record **which** pull request it is waiting on. Today you could write
   `trck set ID --field pr=…` (custom fields already round-trip), but the value is
   opaque text: never validated, never rendered as a link, and invisible in
   `SUMMARY.md` and `trck-html`.

## Decisions

### 1. `in-review` joins the default vocabulary

`DEFAULT_CONFIG` becomes `backlog → ongoing → in-review → done`, and this repo's own
`issues/trck.json` adopts it. Freshly `init`ed trackers get it out of the box.

It carries **no role**. The three lifecycle roles (`initial`/`active`/`terminal`) are
constrained to exactly one status each — that constraint is what the rollup reasons
about, and it stays untouched.

### 2. A new per-status flag: `"actionable": false`

Role-lessness alone doesn't make `ready` skip a status: `is_ready` is
`not terminal and leaf and not blocked`, so an `in-review` leaf would be proposed as
the next thing to work on. Rather than teach the engine the word "review", statuses
gain a generic opt-out:

```json
{ "name": "in-review", "actionable": false }
```

- **Meaning:** "an issue in this status is not available to pick up." Defaults to
  `true`, so every existing vocabulary is unaffected.
- `is_ready` becomes `not terminal and actionable and leaf and not blocked`.
- Any project can now model its own waiting states (`blocked-external`, `qa`,
  `awaiting-deploy`) with no engine change — consistent with the data-driven
  vocabulary rule.
- `validate` gains a light config rule: `actionable`, when present, must be a boolean.

**Deliberately unchanged:**

- **Blocking.** `in-review` is non-terminal, so a dependency on an in-review issue
  still blocks. Correct: the PR is not merged.
- **Rollup.** `reconcile` is untouched, so a parent whose children are in-review rolls
  up to the `active` status (`ongoing`) — not all-initial, not all-terminal. Deriving
  a parent *into* a role-less status would need a general rule for mixed children and
  buys little; out of scope.
- **Icons/colour.** A role-less status already renders `◐` in the active colour, which
  reads correctly for in-flight work; the status column spells out which one it is.

### 3. `pr` becomes a built-in field

Added to `Issue`/`CANON_KEYS` directly after `spec` — both are pointers to an external
document, and the adjacency keeps `show` readable. `FIELD_DEFAULTS["pr"] = None`, so a
row without a PR serializes exactly as before (no index churn on adoption).

- **Value:** an absolute `http(s)` URL, enforced by `PR_URL_RE` at every entry point
  (`new`, `set`, `mv`, `review`) and by `check` for hand-edited rows. Forge-agnostic —
  trck does not know what GitHub is.
- **Setting it:** `new --pr URL`, `set --pr URL|none`, and `mv --pr URL` (record the
  link as part of a move — the same shape as `done --resolution`).
- **Consequence:** `pr` is now reserved, so `--field pr=…` is rejected with the
  existing "use its flag/verb" message. A tracker already using a custom `pr` field
  would see that error; the value still round-trips as a canonical field.

### 4. `trck review ID [URL]`

A third alias verb beside `start`/`done`, driven by a new `aliases` entry
(`"review": "in-review"`), so a tracker that doesn't configure it gets the same
"no 'review' alias configured; use `trck mv`" error `start`/`done` give.

```
trck review 7                                        # -> in-review
trck review 7 https://github.com/o/r/pull/12         # -> in-review, and links the PR
```

It delegates to `cmd_mv` with `status=<alias target>, pr=<url>` — one move, one
`finalize`, one line of output. The optional URL is the whole point of the verb: the
moment a PR exists is the moment both facts are known.

> Aliases remain hardcoded subparsers (`start`/`done`/`review`) rather than generated
> from `trck.json`. Making them data-driven is a worthwhile separate change; it is not
> this one.

### 5. Rendering

- **`show`** — automatic (it iterates `CANON_KEYS`).
- **`SUMMARY.md`** — a `PR: <url>` line under a parent's `Spec:` line, and a
  ` · [PR](url)` suffix on issue rows, via a `pr_tag()` helper beside `label_tag()`.
- **`list`** — unchanged by default (it stays clean). `--show-field` is generalized to
  read `to_dict()` instead of `extra`, so `--show-field pr` works — and so does any
  other canonical field, which is a strictly wider capability at no cost.
- **`trck-html`** — `pr` is exported per issue and rendered as a real anchor in the
  detail pane; a row carries a small `PR` link marker.

## Out of scope

- Data-driven alias verbs (generated subparsers from `trck.json` aliases).
- Deriving a parent's status into a role-less status like `in-review`.
- Fetching PR state from a forge (merged/closed/CI) — trck stays offline and
  stdlib-only.
- Multiple PRs per issue.

## Tests (TDD)

**Config / vocabulary**
- `DEFAULT_CONFIG` contains `in-review` (no role) and the `review` alias; role checks
  still pass.
- `is_actionable` defaults to `True`; returns `False` for an opted-out status.
- `check` errors when `actionable` is a non-boolean.

**ready / next**
- An `in-review` leaf is absent from `ready` and never returned by `next`.
- The same issue moved back to `ongoing` reappears in `ready`.
- An issue depending on an `in-review` issue is still blocked.
- A parent whose only child is `in-review` rolls up to `ongoing`.

**`pr` field**
- `new --pr URL` / `set --pr URL` store it; it round-trips through `index.jsonl`.
- `set --pr none` clears it; a row without a PR serializes with no `pr` key.
- A non-URL value is rejected by `new`/`set`/`mv`/`review` and by `check`.
- `--field pr=…` is rejected as a built-in.
- `show` prints it; `--show-field pr` shows it in `list`.

**`review` verb**
- `review ID` moves to `in-review`; `review ID URL` also sets `pr`.
- Both leave the tracker `check`-clean and produce one output line.
- A config without the `review` alias errors with the `mv` hint.

**Rendering**
- `SUMMARY.md` links the PR for a parent and for a standalone row; a PR-less tracker's
  summary is byte-identical to before.
- `trck-html` exports `pr` and emits an anchor.
