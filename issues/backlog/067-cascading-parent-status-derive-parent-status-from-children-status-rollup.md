# Cascading parent status: derive parent status from children (status rollup)

## Summary
Make a parent's status a derived rollup of its children, the same way `points`
already roll up in `finalize`. Adds a third status role `active`, a
`normalize_statuses` pass, a `manual_status` override (cleared via `set --auto`),
and `check` guarantees. Full design: `docs/specs/2026-06-12-cascading-parent-status-design.md`.

Rule: `all children initial → initial`, `all children terminal → terminal`,
anything in between → `active`. Fully symmetric (covers activation, completion,
and reopen) and recursive to the root, with no per-command hooks because every
verb ends in `finalize`.

## Acceptance criteria
- [ ] `role: active` supported; `active_status(cfg)` helper; this repo's `ongoing`
      gains `role: active`.
- [ ] `normalize_statuses(ctx, g)` derives every non-overridden parent bottom-up
      and is wired into `finalize` alongside `normalize_points`; changed parents go
      through `move_issue` (file relocation + timestamp/resolution handling).
- [ ] `manual_status: bool = False` field added to the model, `CANON_KEYS`, and
      `FIELD_DEFAULTS` (omitted from slim rows when false).
- [ ] A manual `mv`/`start`/`done` on a node with children sets `manual_status`;
      `set --auto` clears it and re-derives.
- [ ] `check`: exactly one status of each role `initial`/`active`/`terminal`; every
      non-overridden parent satisfies `status == reconcile(children)`.
- [ ] `show` displays a `manual_status` marker.
- [ ] Tests per the spec's Testing section (reconcile table, activation/completion/
      reopen/de-activation cascades, new/reparent triggers, override + `--auto`,
      cascaded timestamps, check failures, slim round-trip).

## Notes
- Design discussion resolved: the engine stays vocabulary-agnostic by reasoning
  only about the three roles (never the `start`/`done` aliases, which are pure
  `mv` synonyms). "Not really done" stays a resolution, not a status.
- **Supersedes the approach in #018** (guard + `--recurse` to cascade a manually-set
  terminal status *down* to descendants). This issue inverts that: status is derived
  *up* from children, so a parent can't read terminal while descendants are open in
  the first place — and reopen is included rather than deliberately omitted. #018
  should likely be closed (`wontfix`/`superseded`) once this lands; decision left to
  the owner.
- Out of scope: multiple in-progress statuses; tree/list override markers.
