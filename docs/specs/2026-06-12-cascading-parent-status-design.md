# Cascading parent status (status rollup) — design

**Date:** 2026-06-12
**Status:** approved for implementation

## Why

A parent issue's status should reflect its children. Today they drift: you can
close every child of an epic and the epic still sits in `backlog`; you can start
the first task under a story and the story stays in `backlog` too. Keeping them
in sync by hand is tedious and error-prone.

`points` already roll up — a parent has no own weight; its points are derived
from its children in `finalize`. **Status should roll up the same way.** This
spec adds a status rollup that derives every parent's status from its children,
with an explicit manual-override escape hatch.

## The vocabulary problem (and its solution)

`trck` is vocabulary-agnostic: statuses, priorities, kinds, and resolutions all
come from each tracker's `trck.json`. So the engine cannot hard-code "backlog",
"ongoing", or "done". But it is *already* partially vocabulary-aware: statuses
carry a `role` (`initial`, `terminal`), read through `initial_status()` /
`terminal_statuses()` / `is_terminal()`.

The rollup needs to name three lifecycle positions — where work starts, where it
is in progress, and where it ends. Two already exist as roles. This spec adds the
third:

- **`role: initial`** — exactly one (already enforced conceptually; now checked).
- **`role: active`** — exactly one. **New.** The status a parent rolls up to while
  work is in progress.
- **`role: terminal`** — exactly one.

Aliases (`start`, `done`) are *not* used to carry this meaning: aliases are pure
synonyms for `trck mv` (`cmd_start`/`cmd_done` both delegate to `cmd_mv`), so the
rollup must reason structurally from roles, never from aliases.

"Not really done" outcomes (superseded / duplicate / wontfix) remain expressed
via **resolution**, not via a separate status. A child resolved `wontfix` is still
in the single terminal status, so it still counts toward its parent's completion.
The rollup never reasons about *why* a child is terminal, only *that* it is.

## The rollup rule

For any issue `P` that has children, `P`'s status is a pure function of its
children's statuses:

```
reconcile(P) =
    all children initial   → initial
    all children terminal  → terminal
    otherwise              → active     (any child active, or a partial
                                         mix of initial + terminal)
```

"Work has started but isn't all done" ⇒ `active`, in every form. This is fully
symmetric:

- First child leaves `initial` ⇒ `P` (and its ancestors) become `active`.
- Last child reaches `terminal` ⇒ `P` (and ancestors) become `terminal`.
- A `terminal` child is reopened ⇒ `P` (and ancestors) return to `active`.
- All children pulled back to `initial` ⇒ `P` returns to `initial`.

Recursion is automatic: see "Where it lives" below.

## Where it lives — `finalize`, not per-command hooks

Every mutating command ends in `finalize(ctx, rows)`, which already calls
`normalize_points(Graph(...))` to derive rolled-up points over the whole graph.
Status rollup is implemented the same way:

```python
def finalize(ctx, rows):
    g = Graph(ctx.cfg, rows)
    normalize_points(g)
    normalize_statuses(ctx, g)   # NEW
    save_index(ctx, rows)
    ...
```

`normalize_statuses` walks parents **bottom-up** (children are fixed inputs before
their parent is evaluated; process in reverse-topological / deepest-first order).
For each non-overridden parent whose `reconcile(children)` differs from its current
status, it calls the existing `move_issue(ctx, parent, desired)` — which relocates
the file between status folders and stamps `started` / `closed` / clears
`resolution` exactly as a manual move would.

**Consequence:** there are no per-command hooks. `mv`, `start`, `done`,
`new --parent`, and `set --parent` (reparenting) all get correct, recursive rollup
for free, because each ends in `finalize`. A reopened or newly-added child reopens
its ancestors; the last child done completes them; reparenting reconciles both the
old and the new spine — all from one declarative bottom-up pass.

### Processing order

`normalize_statuses` must evaluate a node only after all its descendants are final.
Compute an order such that every child precedes its parent (e.g. post-order DFS
from roots over the `Graph` child map, or sort nodes by descending depth). The
parent-cycle guard already in `Graph` keeps malformed parent edges from looping;
`normalize_statuses` relies on the same acyclic assumption and must not infinite-loop
on a cycle (skip/short-circuit if a cycle is detected, leaving `check` to report it).

## The manual override — `manual_status`

Pure derivation is simple but inflexible: you could never mark an epic `done`
directly, or pin a parent to a status that disagrees with its children. The escape
hatch is an explicit, recorded opt-out.

- **New field** `manual_status: bool = False` on `Issue`. Added to `CANON_KEYS`
  and `FIELD_DEFAULTS` (`"manual_status": False`) so it is omitted from slim
  `index.jsonl` rows when false, following the existing optional-field convention.

- **Setting it:** a manual `trck mv` / `start` / `done` on a node that has children
  sets `manual_status = True` **only when the requested status differs from
  `reconcile(children)` at that moment** — i.e. only when the move is a genuine
  override of what derivation would produce. A manual move that *agrees* with the
  derived status leaves `manual_status` false (there is nothing to override), so no
  sticky pin is left behind. `normalize_statuses` skips nodes with
  `manual_status == True`, so a real override sticks; the overridden node's status
  still feeds *its own* parent's `reconcile` like any other child. Leaves never get
  the flag — they have no derivation to opt out of.

  This conditional rule is what lets the downward bulk-close in #18 compose with no
  special-casing: `mv X <terminal> --recurse` closes `X`'s leaf descendants *first*,
  so by the time the pin decision is evaluated `reconcile(X.children)` already equals
  the requested terminal status — they agree, so `X` is **not** pinned and instead
  derives to terminal like any compliant parent.

- **Clearing it:** `trck set <id> --auto` sets `manual_status = False`; the
  `finalize` that follows re-derives the node's status from its children and
  cascades upward. `--auto` on a node with no children (a leaf, or already
  un-flagged) is a harmless no-op on the status.

## `check` / `validate` additions

- **Config:** `trck.json` must declare **exactly one** status of each role
  `initial`, `active`, and `terminal`. Missing or duplicated roles is a config
  error. (The `active` role is required of every tracker, including flat ones; the
  scaffolded `trck.json` ships all three.)

- **Data invariant:** every parent with `manual_status == False` must satisfy
  `status == reconcile(children)`. This holds after every verb by construction;
  the check guards hand-edited `index.jsonl` rows and stale data. Parents with
  `manual_status == True` are exempt (any status is valid for an explicit override).

## CLI surface changes

- `trck set <id> --auto` — clear `manual_status`, returning the node to derivation.
  New flag on the existing `set` subparser; composes with other `set` fields.
- `trck show <id>` — display a `manual_status` marker when set, so an opted-out
  node is visibly distinct from a derived one.
- No change to `mv` / `start` / `done` invocation; on a node with children they now
  additionally record the override — but only when the requested status diverges from
  `reconcile(children)` (a move that agrees with derivation records nothing).
- tree/list markers for overridden parents: **out of scope for v1** (YAGNI).

## `trck.json` change (this repo)

```json
{ "name": "backlog", "role": "initial" },
{ "name": "ongoing", "role": "active" },
{ "name": "done",    "role": "terminal" }
```

Only `ongoing` changes (gains `role: active`). Existing issues are unaffected; the
first `finalize` after the change derives any parents currently out of sync (none
should drift in a tracker that has been maintained by hand, but `check` will report
any that do).

## Scope

In scope:

- `active_status(cfg)` helper and `role: active` support.
- `normalize_statuses(ctx, g)` and its wiring into `finalize`.
- `manual_status` field: model, (de)serialization, slim-row omission.
- Override set in `cmd_mv` for nodes with children; clear via `set --auto`.
- `check` config-role validation and the data invariant.
- `show` marker; `trck.json` update for this repo.
- Tests for all of the above.

Out of scope:

- Multiple `active` statuses / multi-stage in-progress workflows. The model is
  exactly one status per role. A future extension could allow extra unroled
  in-progress statuses landing on the single `active` one, without breaking this
  design.
- tree/list overridden-parent markers.
- Any change to points rollup, timestamp stamping, or resolution semantics
  (reused as-is).

## Testing (TDD)

- `reconcile` unit table: all-initial → initial; all-terminal → terminal;
  any-active → active; mixed initial+terminal (no active) → active; single child.
- Activation cascade: starting the first leaf moves parent and grandparent to
  `active`; sibling already active ⇒ no-op.
- Completion cascade: the last child reaching terminal moves parent and grandparent
  to terminal; a non-last child ⇒ no-op.
- Reopen: moving a terminal child back to active reopens the terminal parent and
  grandparent; `closed` / `resolution` cleared via `move_issue`.
- De-activation: moving all children back to initial returns parent to initial.
- `new --parent <terminal-parent>` reopens that parent.
- `set --parent` reparenting reconciles both the old and the new parent spine.
- `manual_status`: a manual `mv` on a parent whose target **diverges** from
  `reconcile(children)` sets the flag and survives `finalize`; a manual `mv` whose
  target **agrees** with `reconcile(children)` leaves the flag false and the node stays
  derived; the overridden status still feeds its own parent's rollup; `set --auto`
  clears the flag and re-derives + cascades.
- Timestamps/resolution on cascaded moves match a manual move (`started` on first
  leaving initial; `closed` set, no resolution, on auto-completion).
- `check`: config missing/duplicating any role fails; a non-overridden parent whose
  status ≠ `reconcile(children)` fails; an overridden parent is exempt.
- Slim-row round-trip: `manual_status` omitted when false, preserved when true.
