# deps: inherit dependencies through the parent hierarchy (effective deps) + lifted cycle prevention

## Summary
Today a dependency is a single authored `depends_on` edge between two issues, and blocking
(`is_blocked`, `trck:216`) only inspects that node's own `depends_on`. But dependencies
should propagate through the **containment hierarchy**: if parent `P2` depends on parent
`P1`, then every child of `P2` is effectively blocked by `P1` (and, since a parent's status
is a rollup of its children — `reconcile`, `trck:202`, issue #ge5jt9s — by all of `P1`'s
subtree too). We want these **effective** dependencies computed at query time from the one
authored edge, never materialized onto children.

The same lifting must gate **cycle prevention**: an effective cycle is a real deadlock, so
mutating verbs must refuse to create one. This is broader than today's `add == self` /
`would_cycle` check on `cmd_dep` — re-parenting (`set --parent`) and creation (`new`) can
also produce an effective cycle.

## Core model

**Storage stays minimal.** Only the authored `depends_on` edge is persisted. Everything
below is derived.

**Lifting rule.** An authored edge `a → b` implies *every node in `subtree(a)` effectively
depends on every node in `subtree(b)`* (because `a` isn't done until `subtree(a)` is, and
"`b` first" means `subtree(b)` first). Equivalently, the effective direct-dependency set of
a node is:

```
effDeps(n) = ⋃ over a ∈ ancestors(n)+n, over authored a→b:  subtree(b)
```

Two equivalent readings of "x effectively depends on y":
- **General/symmetric** (for the deps graph, "why related" queries): `x ⇒ y` iff
  `∃ a ∈ ancestors(x)+x, b ∈ ancestors(y)+y` with an authored edge `a→b`.
- **Blocking (one-sided)**: `x` is blocked iff some `a ∈ ancestors(x)+x` has an authored dep
  on a **non-terminal** `b`. The depended-on side needs no expansion — a parent `b`'s status
  is terminal only when its whole subtree is terminal (rollup), so "wait for `b`" already
  means "wait for `subtree(b)`."

## The disjoint-subtree invariant (ancestor/descendant deps)

A dependency edge `s → d` is admissible **only if `subtree(s) ∩ subtree(d) = ∅`** — neither
node may be an ancestor or descendant of the other (self included). Rationale from the
lifting rule: a single edge `a→b` self-cycles iff `subtree(a) ∩ subtree(b) ≠ ∅`.

- **Same spine (ancestor/descendant) → always forbidden.** `child → parent` gives
  `child ⇒ child` (child waits for parent terminal; parent rollup waits for child) — a real
  deadlock. `parent → child` self-cycles too (`child ∈ subtree(parent) ∩ subtree(child)`).
- **Common parent, disjoint subtrees (siblings / cousins) → always fine.** For `A→B` with
  `A`, `B` both children of `P`, the shared parent `P` is *above* both subtrees and is pulled
  in by nothing; ordering subtasks within an epic is the normal, productive case. A common
  parent by itself imposes no constraint — it only participates in a cycle if it (or a node
  on the shared spine) is itself the endpoint of an authored edge the lift propagates.

This invariant **generalizes** the current `add == row.id` guard (`trck:1685`), which is just
the `s == d` degenerate case. It is a **local, O(depth)** check (walk one spine).

## Cycle prevention — design decision (Option B, chosen)

Every mutating operation is checked against the candidate next-state; an operation that would
create an effective cycle is **invalid and rejected** before anything is persisted. We chose
**verb-level guards** (not a guard inside `finalize`):

- `finalize` (`trck:1368`) stays **write-then-warn** (it `save_index`es then `validate`s and
  warns — `trck:1372–1381`); its "always persists" contract is untouched. This keeps the
  import/`check`/pull paths (`trck:2040`, `:2083`) able to *load* already-cyclic data in
  order to repair it, rather than being locked out by a hard refusal at the seam.
- Interactive mutations `cmd_dep`, `cmd_set` (re-parent via `--parent`), and `cmd_new` call
  a shared guard **before** `finalize`, matching the existing `cmd_dep` pre-check pattern.

Two-tier guard, both sharing the `effDeps` traversal:
1. **Disjoint-subtree check** (local, cheap): reject same-spine edges up front with a clear
   message (e.g. "#B is a descendant of #A; a node can't depend on its own ancestor").
2. **Lifted reachability check**: reject an edge `s→d` when some node in `subtree(d)`
   effectively reaches some node in `subtree(s)` under the closure of `effDeps` — i.e. the
   `child(P1) → child(P2)` deadlock when `P2 → P1` already exists.

Additionally add effective-cycle **detection to `validate`** so `trck check` (and finalize's
warn path) surfaces cyclic data that arrived via hand-edit/import/re-parent done before this
existed. This collapses the earlier "authored cycle = error, effective deadlock = warning"
split into **one rule: an effective cycle is invalid.**

**Presentation caveat:** the cycle lives in *implied* edges, so any message must name the
**authored edges and parent links** responsible — the user never typed the implied loop.

## Acceptance criteria
- [x] `is_blocked` (`trck:216` / `Graph.is_blocked`, `trck:655`) blocks a node when it, **or
      any ancestor**, has a non-terminal authored dependency. Blocking is one-sided (no
      depended-on-side expansion; rollup covers the subtree of the dep).
- [x] A shared `effDeps`-based traversal exists and is reused by the blocking predicate and
      the cycle guard (single source of truth for lifting).
- [x] A local **disjoint-subtree** precondition rejects any `s→d` where one is an
      ancestor/descendant of the other (generalizes and replaces the `add == self` guard),
      with a message naming the containment relationship.
- [x] A **lifted `would_cycle`** rejects edges that close a loop through other subtrees
      (`child(P1)→child(P2)` when `P2→P1` exists).
- [x] Guards run before `finalize` in `cmd_dep`, `cmd_set` (re-parent via `--parent`), and
      `cmd_new` (any op that adds a dep edge or changes the hierarchy). `finalize` remains
      write-then-warn.
- [x] `validate` gains an effective-cycle check so `trck check` reports inherited cycles from
      hand-edited/imported/re-parented data; the message names the authored edges + parent
      links responsible.
- [x] `ready` / `next` reflect effective blocking (a leaf under a blocked parent is not
      "ready").
- [x] Tests (TDD, add failing test first) cover, at minimum:
      - child of `P2` is blocked while `P1` (which `P2` depends on) is non-terminal; becomes
        ready once `P1`'s subtree is terminal.
      - siblings / cousins under a common parent may depend on each other (no false cycle).
      - `child → ancestor` and `ancestor → descendant` deps are both rejected (disjoint-
        subtree invariant); plain `s == d` still rejected.
      - `child(P1) → child(P2)` is rejected when `P2 → P1` exists (lifted deadlock).
      - `set --parent` re-parent that would create an effective cycle is rejected.
      - `trck check` reports an effective cycle present in hand-edited data.
      - existing authored-only behavior (independent subtrees) unchanged.

## Notes
Design context (from discussion):

- **Parent status is a rollup — already live.** `reconcile` (`trck:202`, issue #ge5jt9s),
  applied on every write via `finalize` (`trck:782`). This is *why* the depended-on side of
  blocking needs no expansion, and why `child → parent` deadlocks.
- **Whole-state vs. incremental.** The invariant is a property of the whole index ("the
  effective dependency graph is acyclic"). A whole-graph check on the candidate next-state is
  the correct source of truth and is O(V+E) — trivially cheap here. Keep/derive a lifted
  `would_cycle` only as a targeted guard / better error message; don't hand-encode per-verb
  lifting logic in N places.
- **Why guards belong at the verbs, not in `finalize` (Option B rationale):** a hard refusal
  inside `finalize` would also block the import/repair paths that legitimately need to load
  cyclic data to fix it. Verb-level guards keep interactive mutation strict while leaving
  inspection/repair paths able to surface-and-fix.
- **Cousin cycle example (why lifting is needed for prevention):** `P2→P1` authored, then
  `child(P1)→child(P2)`. `child(P1) ⇒ child(P2)` (authored) and `child(P2) ⇒ child(P1)`
  (via `P2→P1` lifted) — a real mutual deadlock; nothing in either subtree can become ready.
  No false positives: an effective cycle is *always* a genuine deadlock.
- **Re-parenting is `set --parent`, not `mv`.** `cmd_mv` is status-only (moves the file
  between status folders); the hierarchy edge is changed by `cmd_set` via `--parent`. A
  status change can't create an effective cycle, so the hierarchy guard belongs in `cmd_set`
  (which previously ran no dep-cycle check). Re-parenting changes the lifting, so it must run
  the whole-graph `guard_effective_acyclic` before `finalize`.
- **Sibling of #tfhhp8h** (out-of-order completion guard) and **#ge5jt9s** (status rollup).
  Reuse the ancestor walk (`Graph.ancestors_of`, `trck:623`) and `subtree` via
  `children_of` (`trck:614`).
- **Touchpoints:** `is_blocked`/`Graph.is_blocked`, `Graph.would_cycle` (`trck:689`),
  `validate`, `cmd_dep`/`cmd_set`/`cmd_new`, and a new shared `effDeps`/lifting helper on
  `Graph`.
