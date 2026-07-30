# Group the maintenance verbs under 'trck repo'

## Summary
The root namespace carries 20 verbs, and the lifecycle/admin ones crowd out the daily-use ones.
Group the tracker-maintenance verbs under a `repo` parent so `trck --help` leads with the verbs
people actually run.

## Design

**Decision: split by nature.** `trck repo <verb>` for the things you do *to a tracker*; `init`
and `update` stay at the root because they are things you do to your *setup* — `init` is the
first command a new user runs and executes **outside** any tracker, and `update` targets the
engine, not the tracker.

```
trck repo normalize        (was: trck normalize)
trck repo renumber         (was: trck renumber)
trck repo install-hook     (was: trck install-hook)
trck repo migrate-layout   (new — see #x2exfdf)

trck init                  (unchanged — run before a tracker exists)
trck update                (unchanged — targets the engine)
```

**Compatibility:** keep the old flat names working as **hidden aliases** for one release
(`help=argparse.SUPPRESS`, so they vanish from `trck --help` but still run). Drop them in the
release after. `install-hook` in particular is baked into existing `.git/hooks/pre-commit`
scripts and CI configs, so a hard break would silently stop validating trackers.

**Rejected alternatives:**
- *`trck admin <verb>` for all six.* Simplest grouping, but it buries `init` — the one verb a
  first-time user needs — and lumps together two genuinely different kinds of command. The split
  above is the only one that reflects a real distinction rather than "stuff I use less".
- *Keep flat, hide from `--help` with `SUPPRESS`.* Zero breaking change and it does solve the
  visible clutter, but the namespace keeps growing and every future maintenance verb makes the
  problem worse. Grouping is the structural fix.
- *Group with no aliases.* Cleanest end state, but breaks committed pre-commit hooks with no
  grace period.

`argparse` nests directly: `repo = sub.add_parser("repo"); rsub = repo.add_subparsers(dest="repo_cmd", required=True)`.

## Acceptance criteria
- [ ] `trck repo normalize|renumber|install-hook` work and are listed under `trck repo --help`
- [ ] `trck --help` no longer lists them at the root; `init`/`update` still appear there
- [ ] The old flat names still work, hidden from `--help`
- [ ] `trck install-hook` writes a hook that invokes the **grouped** form, so newly-installed
      hooks don't depend on the deprecated alias
- [ ] Tests cover both spellings for each moved verb
- [ ] README and `CLAUDE_MD_TEMPLATE` verb lists updated

## Notes
Blocks #x2exfdf so `migrate-layout` is born as `trck repo migrate-layout` and never needs
renaming in a released CLI. That is the only hard ordering between this and the layout epic
(#2srvf6j) — the rest of the epic is independent.

Deprecation of the flat aliases is deliberately *not* in this issue's scope; file a follow-up
when the removal release is chosen.
