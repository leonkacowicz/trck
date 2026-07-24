# list: 'needs' annotation ignores inherited deps, disagreeing with ready/is_blocked

## Summary

`block_annotations` builds its dim ` needs #…` suffix from `Graph.requires_of(r)`, which
reads **only** the row's own authored `depends_on`. Its own docstring claims the suffix
"mirrors `is_blocked`" — but `is_blocked` reads `lifted_deps`, i.e. the row's deps **plus
every ancestor's**.

The two views therefore disagree. A child that is blocked solely by a dependency authored
on its parent renders with **no** `needs` annotation in `list`, looking actionable, while
`ready`/`next` correctly hide it. The row view says "go", the readiness view says "wait".

This is the inline surface for the one genuinely subtle rule in the model — dependencies
climbing the hierarchy — so it's exactly where a silent omission costs the most.

## Acceptance criteria
- [x] `needs` reflects **effective** (hierarchy-lifted) dependencies, matching `is_blocked`.
- [x] A child blocked only through an ancestor's `depends_on` shows a `needs #…` suffix.
- [x] Inherited entries are distinguishable from the row's own authored deps, so the
      annotation doesn't imply the edge was authored on this row (drives where you'd run
      `trck dep --remove`). Exact presentation TBD — see Notes.
- [x] `blocks` is reviewed for the mirror-image gap: `dependents_of` is likewise
      authored-only, so an issue depended on via an ancestor may under-report what it blocks.
- [x] Regression test covering parent-authored dep → child row annotation.
- [x] `block_annotations`' docstring matches what the code actually does.

## Notes

- `trck:1147` `block_annotations`, `trck:1152` the `requires_of` call, `trck:610`
  `requires_of` (authored-only), `trck:674` `is_blocked` (lifted).
- Fixing this is likely a one-line swap to `lifted_deps`, but decide the presentation
  first: an undifferentiated merge makes an inherited constraint look locally authored.
  Options: a distinct verb (`needs` vs `inherits`), or naming the ancestor
  (`needs #X (via #P)`).
- Found while designing the inferred-edge work for the deps graph (parent→child
  containment edges + inheritance edges + transitive reduction). One option there was to
  leave inheritance out of the drawn graph and let this annotation carry it — that option
  is unsound until this is fixed, so this issue gates that design choice.

## Resolution

`needs` now reads `lifted_dep_sources` — each effective blocker paired with the issue that
authored the edge — so it agrees with `is_blocked`/`ready` in every view.

Presentation, chosen for **compactness over restatement**:

- An inherited blocker renders as `needs #B (via #P)`, naming the author (where
  `dep --remove` goes), never the row itself.
- It is spelled out **only when no row between this one and the author is being printed**.
  In the usual nested forest the parent sits right above its children already saying
  `needs #B`, so the children stay quiet; `--flat` with a filter that drops the parent, or
  a forest rooted below it, spell it out. `block_annotations` takes the `on_screen` id set
  for this; `cmd_list` passes the rows it is about to render.
- Reported once down a spine: the topmost printed descendant of the author carries it.
- **`blocks` deliberately stays authored-only** (criterion 4's review outcome). The authored
  dependents *are* the maximal elements of the lifted dependent set, so nothing is lost —
  it just stays at the altitude the edge was authored. Mirroring the lifting would fan a
  blocker of a 20-child parent out to 21 ids per row.

Does **not** unblock the "let the annotation carry inheritance" option for #dj5b42j's
graph work: `deps` renders via `node_label` and never shows this note, and the suppression
rule makes the note absent precisely when the parent is on screen — which in a drawn graph
it always is. #zbkkc2a should still draw inheritance edges on its own terms.
