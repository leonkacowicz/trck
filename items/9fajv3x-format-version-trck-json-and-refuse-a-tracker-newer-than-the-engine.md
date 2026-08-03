# format: version trck.json and refuse a tracker newer than the engine

## Summary
`trck.json` carries no format version and nothing guards one, so today the vendored copy *is*
the pin. Un-vendoring without this lets an old engine and a new tracker meet silently.

Half the problem is already solved: `Issue.extra` round-trips unknown index keys verbatim, so an
old engine rewriting a row containing a field it has never heard of preserves it. The unguarded
surface is **config**. `load_config` merges `trck.json` over the defaults and ignores keys it
does not know — and there is a live example. A status carrying `"actionable": false` is read by
any engine predating that feature, ignored, and its issues are then offered by `ready`/`next`.
Wrong answers, no error, no way to notice.

## Acceptance criteria
- [ ] A `format` integer in `trck.json`; absent means the current shape. `SUPPORTED_FORMAT` in
      `constants.py`.
- [ ] One guard in `load_config` — every verb passes through it — refusing a tracker whose
      `format` exceeds the engine's, naming the fix. Refuse newer only; older is what migration
      is for.
- [ ] Extensions, not just an integer. A flat version pins the whole tracker, so bumping it for
      something like `actionable` would lock out old engines even for repos not using it. Git's
      model: the version says "you may meet extension keys, refuse any you do not know", giving
      per-feature granularity.
- [ ] A written bump policy. Because `extra` round-trips, additive fields need no bump — a bump
      is for changes that make an old engine *wrong*, not merely ignorant. Both historical
      breaks would have qualified: status-folders → `items/`, and integer → random ids.
- [ ] `trck init` writes it; `trck check` validates it.
- [ ] Tests: newer refused, equal and absent accepted, unknown extension refused, known
      extension accepted, and the existing unknown-key round-trip pinned.

## Notes
Small — call it 150–200 lines with tests. The design content is the granularity choice, not the
code. Needed regardless of the rewrite.
