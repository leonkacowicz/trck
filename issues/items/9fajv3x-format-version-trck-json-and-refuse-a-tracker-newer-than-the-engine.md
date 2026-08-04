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
- [x] A `format` integer in `trck.json`; absent means the current shape. `SUPPORTED_FORMAT` in
      `constants.py`.
- [x] One guard in `load_config` — every verb passes through it — refusing a tracker whose
      `format` exceeds the engine's, naming the fix. Refuse newer only; older is what migration
      is for.
- [x] Extensions, not just an integer. A flat version pins the whole tracker, so bumping it for
      something like `actionable` would lock out old engines even for repos not using it. Git's
      model: the version says "you may meet extension keys, refuse any you do not know", giving
      per-feature granularity. `KNOWN_EXTENSIONS` is empty — the mechanism ships, no extension
      does.
- [x] A written bump policy. Because `extra` round-trips, additive fields need no bump — a bump
      is for changes that make an old engine *wrong*, not merely ignorant. Both historical
      breaks would have qualified: status-folders → `items/`, and integer → random ids.
      Written beside `SUPPORTED_FORMAT` (authoritative) and as a table in the README.
- [x] `trck init` writes it; `trck check` validates it — by building a Ctx like every other
      verb, so there is no second format check to keep in sync.
- [x] Tests: newer refused, equal / older / absent accepted, malformed refused cleanly, unknown
      extension refused (all of them named, not just the first), known extension accepted, a
      mutating verb refused as well as a read-only one, and the unknown-key round-trip given
      the docstring that ties it to the bump policy.

## Notes
Small — call it 150–200 lines with tests. The design content is the granularity choice, not the
code. Needed regardless of the rewrite.

## Decided while building

**`update` must be exempt from the guard.** It resolves its repo through `build_ctx`, so putting
the guard in `load_config` — correct for every other verb — would have made the refusal
self-defeating: the message says "run `trck update`", and `trck update` would then have refused
too, leaving no way to get an engine that understands the tracker. `_update_repo` now passes
`guard_format=False`, which is safe because it reads only `update.repo`, a string in every
format. This is the one place a new verb could get the guard wrong, which is why it carries a
comment rather than just a parameter.

**The bootstrap limit, stated rather than papered over.** The guard protects engines from this
release forward; one older than it ignores both `format` and `extensions` and can still be
fooled by exactly the `actionable: false` case in the Summary. Nothing can fix that
retroactively. It does mean un-vendoring (`djx63gk`) is only safe once an installed engine is
guaranteed to be ≥ this version, which is a real precondition on that issue rather than a
detail — the vendored copy is still the pin until then.
