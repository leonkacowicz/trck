# Merge tree into list: structure-aware browse verb

## Summary

We have several presentation verbs (`list`, `tree`, `ready`, `next`). `list` and `tree`
differ on exactly one axis — **flat vs. nested** — while everything else `list` offers
(filtering, sorting) is orthogonal and would apply equally well to a nested view. Today:

- `list` shows the flat universe of all issues (filterable, globally sorted); a child only
  hints at its parent via a dim `↳NNN` tag.
- `tree` shows only issues that participate in an epic→child hierarchy (standalone issues are
  invisible), with no filtering and no sorting.

**Goal:** collapse them into one structure-aware browse verb. `list` shows a **nested forest
by default** — every issue, children nested under their parent — while keeping all of `list`'s
filtering and ordering. Reordering only ever moves a child *within* its parent, never out of it.
`ready`/`next` stay as-is: they're a ranked work-queue query, a different species from browsing.

### Design (decided)

- **Verb shape.** `list` renders the nested forest by default. `list --flat` recovers today's
  flat, globally-sorted list. `list <id>` (optional positional, inherited from `tree`) roots
  the forest at one issue (it + its subtree). `tree` becomes a plain **alias for `list`**, so
  `trck tree 040` → `trck list 040`.
- **Row layout** (one shared renderer for flat and nested, so they stay consistent — today's
  `list` column order is unchanged):

  ```
  ● #040 ongoing  high    Auth epic
  ◐ #042 backlog  medium  ├─ Login form
  ○ #051 ongoing  high    │  └─ Validate token
  ● #043 done     low     └─ Logout
  ```

  Order: status icon · bold `#id` · status word (colored) · priority word (full, colored) ·
  indentation-marker + title · then existing `[EPIC]`/label tags and the dim
  `needs #… blocks #…` annotations. The **only** difference between flat and nested is the
  connector prefix in front of the title (empty in `--flat`).
- **Filtering = matches + their ancestor spine.** A node is shown iff it matches the filter
  **or** has a descendant that matches. Shown-but-non-matching ancestors render **dimmed** as
  context, so a matched child never floats away from its parent. All current filters keep their
  meaning (`--status/--kind/--priority/--label/--match/--blocked/--orphan/--parent`); they
  select the match set and the tree fills in ancestors.
- **Sorting = within sibling groups.** `--sort` (priority/points/id/created, default id) orders
  siblings among themselves, recursively — a child can only reorder within its parent. In
  `--flat` mode, sort stays global as today.
- **Roots & edge cases.** Top level = `parent is None`. An issue whose `parent` points at a
  missing id is treated as a root (can't nest under nothing); `check` still flags the dangling
  ref separately. Cycle-safe via `walk_tree`'s existing `seen` guard (`(cycle)` then stop).

### Implementation sketch

- One renderer computes status/priority column widths, then per row emits
  `icon · #id · status · priority · <connector-prefix>title · tags · annotations`. `--flat`
  passes an empty prefix; nested passes `walk_tree`-style connectors.
- Nested path: build `kids_of`, compute the match set + ancestor closure, sort each sibling list
  by the chosen key, walk from roots, dim non-matching ancestors. Keep `block_annotations` wired
  through both paths.
- `tree` → alias to `list` (carry the optional positional `id`). Update README/help text.

## Acceptance criteria

- [ ] `trck list` (no args) prints a nested forest: every issue, children nested under parents.
- [ ] `trck list --flat` reproduces today's flat, globally-sorted output (column order unchanged).
- [ ] Row layout is `icon · #id · status · priority · indent+title · tags · annotations`, with
      full colored status/priority words; flat and nested share the renderer.
- [ ] Filtering shows matched nodes plus their ancestor spine; non-matching ancestors are dimmed.
- [ ] `--sort` reorders siblings only, recursively (a child never escapes its parent).
- [ ] `trck list <id>` roots the forest at that issue's subtree.
- [ ] `trck tree` is an alias for `list` (`trck tree <id>` == `trck list <id>`).
- [ ] `ready`/`next` are unchanged.
- [ ] Dangling-parent and dependency cycles render without crashing.
- [ ] Tests cover each of the above; `trck check` passes; help text and README updated.

## Notes

- Engine functions in play: `cmd_list`, `print_rows`, `cmd_tree`, `walk_tree`, `node_label`,
  `block_annotations`, `parse_status_filter`, the `list`/`tree` argparse blocks.
- Open follow-up (out of scope here): `ready`/`next` could later be reframed as `list` presets,
  but their leaf+unblocked logic is a distinct concept — left alone for now.
