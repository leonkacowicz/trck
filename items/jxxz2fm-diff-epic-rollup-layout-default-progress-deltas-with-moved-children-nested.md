# diff: epic-rollup layout (default) — progress deltas with moved children nested

## Summary
The default body of `trck diff`, and the layout that is distinctive to trck rather than borrowed
from git: group changed issues under their epics and lead each group with the epic's **progress
delta**, using the rollup already computed for `list` and `SUMMARY.md`. The epic line carries the
meaning; the children carry the evidence.

**This is not a new renderer.** It is `cmd_list`'s nested-forest path with three substitutions:

| `cmd_query.py` | today | for diff |
|---|---|---|
| `shown, dim = g.match_closure(keep)` | status/priority/match filters | `keep` = "changed" (below) |
| `progress = lambda r: progress_pct(g, r)` | one-sided `NN%` | a two-sided delta |
| the status column in `print_rows` | `r.status` | a transition cell, widened |

`match_closure` (graph.py:112) already returns matched rows **plus their dimmed ancestor spine**, so
the tree scaffolding is free. `forest_layout` supplies the connectors; `print_rows` already accepts
`prefix` / `dim` / `progress` / `abbrev`. `leaf_rollup` is pure over a `Graph`, so the old side is
just `Graph(cfg, old_rows)`.

```
  ◐ #fkrp9dh  in-progress         medium  trck-html: static HTML SPA  60% → 90% (20 → 28 pts) [EPIC]
  ● #4vbk79y  in-progress → done  low     ├─ v4: tree / hierarchy view with rolled-up progress
  ● #6yttx97  in-progress → done  low     ├─ v3: dependency graph view
+ ○ #tcm5s56  new             low     └─ v7: trck serve — live process
  ◐ #d9ckqzc  done ↩ in-progress  medium  trck-html: highlight shortest-unique id prefix
- ○ #x9y8z7w  removed         low     obsolete thing
  ○ #wh3mv52  backlog         medium  mv/done: guard closing a parent   priority low → medium
```

## Settled design

**Slots.** `[gutter] [icon] #id  <status transition>  <priority>  <connectors><title> <progress
delta>  <chips>`. The status transition goes in the (widened — `sw` is data-derived, so this is free)
status column; the progress delta lands in `print_rows`'s existing `prog` slot right after the title;
every other field change becomes a trailing chip. Priority stays a plain current-value column —
two transition columns is one too many.

**Gutter.** `+` and `-` only; a modified row's gutter is blank. The arrow already says "modified",
and modification is the common case, so marking it would put a sigil on nearly every line. Reserving
the gutter means a non-blank gutter always signals *something structural happened*. The status icon
is kept and shows the **new** state.

**Direction must survive monochrome.** A backward move renders `done ↩ in-progress`, not just a
differently-coloured `→`. Colour is an enhancement here, never the sole carrier — `NO_COLOR` and
pipes are first-class.

**The `keep` predicate is `own row changed OR rollup changed`.** The second clause is essential.
With only the first, an epic whose children all closed does not match (its own row is untouched —
title, priority same, status derived); it enters `shown` merely as an ancestor, so it lands in `dim`
and renders grey — putting the `60% → 90%` headline in grey above three bright children. With the
clause, the epic matches on its own merit and renders bright.

The dim spine stays meaningful rather than becoming vacuous: a metadata-only change (priority,
label, title) moves no rollup, so ancestors correctly stay dim as pure context. Status and points
changes light them up. That is exactly the distinction the layout should draw.

**Progress delta format.** `60% → 90%`, with a points marker appended **only when the denominator
moved**: `60% → 90% (20 → 28 pts)`. Percent alone is ambiguous — it reads the same whether work was
finished or unfinished children were deleted. Both numbers come from the same `leaf_rollup` call, so
the marker costs no extra traversal. This also explains an otherwise alarming case: an epic that
loses a child to re-parenting shows `90% → 75% (28 → 20 pts)`, and the marker names the cause.

**Union graph for layout.** Build the forest from a union of both sides: the new row per id, falling
back to the old row when an id is absent from the new side. Deleted issues then sit at their
last-known parent under their real id — no trailing orphan section needed.

**Re-parented issues appear twice**, as a departure at the old position and an arrival at the new
one, keyed by a **shadow id**: the departure row is inserted with id `~abcdef` and the *old* parent.
`~` is not in the base32 id alphabet, so a shadow can never collide with a real id, and the renderer
strips the `~` when printing. Shadows are needed **only** for re-parenting — a genuine deletion has
no id conflict and uses its real id. So `~` means precisely "departure record".

The departure row is **dimmed and carries only the move** (`moved to #u5fc5vm`); the arrival row is
the sole full record of what else changed. Symmetry would mean reading the same status transition
twice.

```
  ◐ #fkrp9dh  in-progress         medium  trck-html: static HTML SPA  90% → 75% (28 → 20 pts) [EPIC]
- ● #zssaj4k                  medium  └─ trck-html: graph view    moved to #u5fc5vm      ← dimmed
+ ○ #u5fc5vm  new             medium  trck diff: semantic diff of tracker state  0% [EPIC]
+ ● #zssaj4k  in-progress → done  medium  ├─ trck-html: graph view    moved from #fkrp9dh
```

Three places must know about shadows:
- `unique_prefix_lens` — compute over real ids only; a shadow looks up its stripped id. Otherwise a
  re-parented id is counted twice and every highlighted prefix gets longer for nothing.
- `block_annotations` — suppress on shadows. A tombstone is not a work item and must not claim to be
  blocked.
- `leaf_rollup` — **never** runs over the union graph. Deltas come from `Graph(cfg, old_rows)` and
  `Graph(cfg, new_rows)` separately; the union graph is a layout device only, and rolling up over it
  would double-count the shadow.

Shadow ids exist in memory for the duration of one render. They are never written to `index.jsonl`.

**Loose issues need no special handling** — a parentless changed issue is a forest root, exactly as
in `list`. No separate section.

**Nesting depth: the full ancestor spine**, dimmed, as `match_closure` already produces. No
collapsing, no breadcrumbs.

## Acceptance criteria
- [ ] Changed issues are grouped under their epic via `match_closure`; parentless changes render as
      forest roots.
- [ ] `keep(r)` matches on own-row change **or** rollup change; an epic whose children moved renders
      bright with its delta, and a metadata-only ancestor renders dim.
- [ ] Epic rows show `old% → new%`, with `(old → new pts)` appended iff the points total changed.
- [ ] Deleted issues render under their last-known parent; re-parented issues render at both
      positions, the departure dimmed with a `moved to #…` chip and the arrival with `moved from #…`.
- [ ] Shadow (`~`-prefixed) ids never appear in output, never reach `index.jsonl`, and are excluded
      from `unique_prefix_lens` and `block_annotations`.
- [ ] Backward transitions are distinguishable with colour disabled.
- [ ] This is the default layout; `--flat` selects the ledger (#yfxtkd8) instead.
- [ ] Existing `list` output is byte-identical to before the change.

## Notes
- Depends on the change model (#u8qaqwr) for the per-issue records and status-direction
  classification.
- Minor, still open: sibling sort order for the diff forest (reuse `list`'s `created` default, or
  sort by magnitude of change?). Default to matching `list` unless it reads badly in practice.
- The shadow-id trick is the one place this layout cannot reuse `list`'s machinery verbatim, because
  `match_closure` and `forest_layout` key on id and one id now occupies two forest positions.
