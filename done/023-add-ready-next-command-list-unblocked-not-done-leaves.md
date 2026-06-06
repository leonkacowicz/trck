# Add 'ready'/'next' command: list unblocked, not-done leaves

## Summary
Surface the single query that makes maintaining a dependency graph worthwhile:
"what can I work on right now?". An issue is *ready* when it is not in a terminal
status, is a leaf (no children), and every issue it depends on is in a terminal
status. The dep graph, leaf detection, and points are already tracked — nothing
currently exposes this view.

`trck ready` lists all ready issues, sorted by priority (highest first) then by
points. A `trck next` form (or `ready --next`) prints just the top pick.

## Acceptance criteria
- [x] `trck ready` lists not-done leaves whose deps are all in a terminal status.
- [x] Output is sorted by priority, then points, then id.
- [x] `next` (or `ready --next`) prints only the single highest-ranked ready issue.
- [x] Issues with at least one non-terminal dep are excluded (and don't show in `ready`).
- [x] Epics/parents (non-leaves) are excluded.
- [x] Honors the configured terminal status role(s), not a hardcoded "done".
- [x] Tests cover: unmet dep, met dep, no deps, parent excluded, ordering.

## Notes
Relates to the existing `deps` graph and leaf/rollup logic. Terminal status comes
from `trck.json` roles, like the points rollup already does.
