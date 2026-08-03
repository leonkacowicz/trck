# repo: migrate an existing tracker to the fixed vocabulary and the new format

## Summary
Existing trackers carry free-form statuses, priorities and resolutions, and no `format` key.
They need a one-shot migration, following the pattern `repo migrate-layout` already set: a verb
that is idempotent, previewable with `--dry-run`, and refuses to write when it cannot map
something unambiguously.

## Acceptance criteria
- [ ] Maps each configured status onto a semantic state, using `role`/`actionable` where present
      and asking rather than guessing where not.
- [ ] Maps priorities onto the five canonical levels, keeping the old names as display aliases
      so nothing changes on screen.
- [ ] Rewrites `index.jsonl` rows to canonical values, preserving `extra` untouched.
- [ ] Writes the `format` key.
- [ ] Aborts without writing on anything ambiguous — a status with no role and no obvious
      mapping, or more priorities than slots.
- [ ] Every verb refuses to run on an unmigrated tracker, naming the fix, exactly as the layout
      migration does.
- [ ] Dogfooded on this repo's own tracker.
