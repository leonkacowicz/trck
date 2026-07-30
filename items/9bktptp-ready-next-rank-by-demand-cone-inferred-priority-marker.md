# ready/next: rank by demand cone (inferred priority) + marker

## Summary
`ready`/`next` sort by an issue's *declared* priority, which understates work that
unblocks something hotter. If `A` (urgent) depends on `X` (medium) and `B` (medium)
depends on `Y` (high), `ready` puts `Y` above `X` — but `X` is the thing standing
between us and an urgent issue.

Rank instead by the **demand cone**: `X` plus every non-terminal issue that
transitively waits on `X`. Both propagation channels count — an authored dependency
(a dependent needs `X`) and containment (a parent is not done until `X` is).

## Acceptance criteria
- [ ] `Graph` exposes the demand cone and a priority-count vector over it
- [ ] `ready`/`next` order by that vector, then the existing `-points`, `id` tie-breaks
- [ ] a row whose cone outranks its own priority is marked `↑<priority>(#culprit)`
- [ ] nothing is written back to `index.jsonl` — the ranking is fully derived
- [ ] README, `--help` epilog, and the AGENTS scaffold describe the ranking

## Notes
**The key.** Count the cone's members per configured priority and compare the counts
lexicographically, highest priority first:

    key = (-n_urgent, -n_high, -n_medium, -n_low, -n_lowest, -points, id)

One key covers both rules we wanted. The first non-zero slot *is* the max priority in
the cone, so "blocks an urgent issue" beats "is high". Within a slot, blocking two high
issues beats blocking one. Levels never trade against each other — fifty mediums do not
outrank one high — which keeps the ordering explainable.

**The relation is effective blocking, reversed.** `is_blocked` (and so `ready` itself)
uses the lifted relation: `X` blocks `r` iff some ancestor-or-self of `r` authored a
dependency on some ancestor-or-self of `X`. The demand cone is the transitive closure of
its reverse, which per authored edge `a -> b` means every member of `subtree(b)` is
demanded by every member of `subtree(a)`. Plus the containment edge: a node is demanded
by its parent. Terminal issues neither count nor conduct — an urgent issue closed as
`wontfix` no longer makes its blockers urgent.

**Why containment counts.** An urgent epic makes its own leaves urgent without anyone
authoring a dependency edge, which is the common case in this tracker.

**No ordering regresses.** A declared-priority sort is the degenerate case of this one
(an empty cone), so the ranking is always on — no config flag. The marker is what keeps
it honest, since otherwise `ready` shows a `medium` above a `high` with no visible
reason.

**Out of scope.** `list --sort priority` and `SUMMARY.md` keep sorting on the stored
field; a generated doc grouped by a graph-derived value would churn on unrelated edits.
`trck-html` parity is #me67zba. Later refinements, if the vector leaves real ties:
unlock-count (how many issues become ready) and critical-path length.
