# summary: sort status sections by priority (highest first)

## Summary
The per-status lists in `SUMMARY.md` (Backlog / Ongoing / Done) currently sort
standalone issues by `id`. Within each status section they should instead sort by
**priority, highest first** (using the configured `priorities` order, where index 0 is
highest), falling back to `id` as a stable tiebreaker. This surfaces the most important
open work at the top of each section.

## Acceptance criteria
- [x] Standalone issues in each status section of `SUMMARY.md` are ordered by priority
      (highest configured priority first), then by `id` ascending within the same priority.
- [x] The ordering follows the configured `priorities` list rather than hard-coding
      high/medium/low; an unknown priority sorts after all known ones.
- [x] `trck check` passes and `SUMMARY.md` regenerates cleanly.

## Notes
- Scope is the `standalone(status)` helper in `generate_summary` (the `## Backlog` /
  `## Ongoing` / `## Done` sections). The Hierarchies section keeps its current
  milestone/id ordering.
- A small `priority_rank(cfg, prio)` helper mirrors the existing `priority_codes`.
