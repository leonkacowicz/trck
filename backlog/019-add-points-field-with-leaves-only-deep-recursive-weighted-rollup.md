# Add points field with leaves-only deep-recursive weighted rollup

## Summary
Give each issue a `points` weight and make the SUMMARY rollup report a points-weighted
completion percentage, aggregated recursively down to the leaves. Today the rollup is a
shallow count of direct children (`ndone/len(kids)`); this makes a parent's percentage
reflect the real size and depth of the work under it.

**Core rule:** `points` is a *leaf-only input*. A node with children has no stored
`points`; its weight is the sum of its leaf descendants, derived live and never stored.

## Acceptance criteria

### Field & storage
- [ ] New per-issue field `points`: a **non-negative integer**, default `1`.
- [ ] `points` is stored **only on leaves** (issues with no children). On every index
      write, a node that has children has its `points` stripped (absent/null) — normalized
      centrally in `finalize`, so the leaf→parent transition is handled no matter which
      command caused it.
- [ ] A node that loses its last child becomes a leaf again with no stored points, and
      therefore falls back to the default `1`. The previous value is not preserved (git
      history is the audit trail).
- [ ] Added to `CANON_KEYS` (right after `priority`) so `show` renders it for leaves; a
      parent (null) shows no points line, per the existing skip-empty rule.

### CLI
- [ ] `trck new "<title>" [--points N]` — default `1`; reject `N < 0`.
- [ ] `trck set NNN --points N` — reject `N < 0`; **error if the issue has children**
      ("points is derived for issues with children"), rather than silently accepting a
      value that `finalize` would strip.

### Validation
- [ ] `validate()` errors if a leaf's `points` is not an int or is `< 0`.
- [ ] `validate()` errors if a node with children carries a non-null `points` (signals a
      stale/hand-edited index).

### Rollup (generate_summary — the one parents loop, covers epics and non-epic parents)
- [ ] A parent's totals are computed by recursing to leaf descendants, with a `seen`
      cycle guard (so `summary` is safe even on an index that `validate` would reject):
      - `ptotal` = Σ points over leaf descendants
      - `pdone`  = Σ points over leaf descendants whose status is terminal
      - `ntotal`/`ndone` = count of all / terminal leaf descendants
      - `pct = round(100 * pdone / ptotal) if ptotal else 0`
- [ ] Heading line shows points **and** count:
      `### [#NNN title] — {pct}% ({pdone}/{ptotal} pts · {ndone}/{ntotal} done) · _prio_ · status`
- [ ] With all-default points (every leaf = 1), `pct` equals the old count-based
      percentage — existing output is unchanged until points are assigned.
- [ ] Direct-child bullets under each heading are unchanged (each shows its own
      `[x]`/status). Every sub-parent still gets its own heading with its own subtree pct.

### Tests (TDD)
- [ ] `new --points` stores the value; default `1`; rejects negative.
- [ ] `set --points` updates a leaf; rejects negative; errors on a parent.
- [ ] `finalize`/write strips `points` from a node once it gains children; a former
      parent that becomes a leaf reads as default `1`.
- [ ] `validate` flags a negative/non-int leaf points and a non-null parent points.
- [ ] `generate_summary`: weighted pct uses leaf points; deep (grandchild) leaves are
      summed; `ntotal`/`ndone` are leaf-based; all-default points reproduces the
      count-based pct; `ptotal == 0` guard yields `0%`; cycle guard terminates.

## Notes
Design decided in discussion (2026-06-05):

- **Leaves-only, to avoid double-counting.** For epic E → A (5) and sub-epic B → B1 (2),
  B2 (3), E's total is A+B1+B2 = 10. Counting B's own points too would double-count B.
  Equivalently: "sum of descendants" and "sum of leaf descendants" are the same number,
  so the recursion composes at every level.
- **No dormant numbers.** A parent's stored points would never be counted, so we delete
  it rather than keep it. `effective` weight is derived live from leaves. Recovery, if
  ever needed, is `git log`/`git show` on the issue files.
- **Consequence:** a parent's own status no longer affects its rollup percentage (only
  leaves do). Marking an epic `done` while leaves are open shows < 100% — more honest.
  The separate concern of *blocking/cascading* such a close is tracked in **#018**, which
  is independent of this issue (rollup math vs. mv/done semantics — keep commits separate).
- One shared recursive helper feeds the rollup tallies (and could feed a derived `show`
  display later, deliberately out of scope here — the summary owns the derived number).
- Backfill: the 17/18 existing issues predate the field. On the first write, leaves gain
  `points: 1` and current parents (e.g. the #002 epic) have it left absent — no separate
  migration step needed.
