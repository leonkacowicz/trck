# v4: tree / hierarchy view with rolled-up progress %

## Summary

Add a third view to the SPA: a **tree / hierarchy** forest, mirroring `trck tree` /
SUMMARY. The `[ list | graph ]` toggle becomes `[ list | graph | tree ]`. Parents nest
their children; parent rows show a **rolled-up progress %** (the same points-weighted
number the engine reports); rows are collapsible and clickable → detail panel.

## Design

**Data (testable Python seam):** `build_model` gains `model["roots"]` — the id-sorted
top-level issues (`parent is None`), the forest entry points. Children come from the
existing per-issue `children`, and the rolled-up `progress` (pct + points) already rides
each parent issue (from the engine's `leaf_rollup`). No new rollup logic — v4 just
surfaces what the model already carries.

**View (JS; browser-verified):**
- A `tree` button joins the view toggle; selecting it shows a nested forest in the left
  pane (same slot as list/graph).
- Rendered recursively from `roots` → `children`: each parent row has a ▾/▸ caret to
  collapse/expand its subtree (collapsed set held in state); leaves have no caret.
- Parent rows show `pct%` and a thin progress bar; done leaves are dimmed/struck like the
  list. Clicking any row → `select(id)` (detail panel updates, row highlighted).
- The full forest is shown (including done subtrees) for this MVP; a "hide done" toggle
  is a later polish. Children keep the engine's id-sorted order.

## Acceptance criteria

- [ ] `model["roots"]` lists id-sorted top-level issue ids (parent-less); children excluded.
- [ ] Parent `progress` rolls up correctly (e.g. one of two equal-weight children done → 50%);
      leaves carry `progress: null`.
- [ ] Rendered document includes the tree container + the `tree` view toggle.
- [ ] Tree view renders a collapsible forest with progress on parents; rows select on click.
- [ ] v1–v3 behaviour unchanged; full suite + `build.py --check` green.

## Notes

The recursive tree render + collapse interaction are client-side (browser-verified); the
Python tests cover the `roots` contract and the rolled-up `progress` the view depends on.
"Hide done" and priority-ordering are deferred. Parent: epic #fkrp9dh.
