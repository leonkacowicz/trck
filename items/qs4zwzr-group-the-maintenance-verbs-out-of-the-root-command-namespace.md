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

**Compatibility: a clean break — no aliases, hidden or otherwise.** The old flat spellings are
removed outright. The point of this change is to *organize* the namespace, and keeping the old
names alongside the new ones perpetuates exactly the clutter being removed while doubling the
surface to document and test. trck is pre-1.0, and v0.23.0 is already a breaking release (the
on-disk layout) — one breaking release is better than two.

**Checked, and it is cheaper than it looks:** the pre-commit hook `install-hook` generates
invokes `trck check`, which is **not** moving. So committed `.git/hooks/pre-commit` scripts keep
working untouched. Only the hook's header comment (`# Auto-installed by \`trck install-hook\``)
goes stale and needs updating to the grouped spelling.

**Rejected alternatives:**
- *`trck admin <verb>` for all six.* Simplest grouping, but it buries `init` — the one verb a
  first-time user needs, and the only one that runs *outside* a tracker — and lumps together two
  genuinely different kinds of command. The split above is the only one reflecting a real
  distinction rather than "stuff I use less".
- *Keep flat, hide from `--help` with `SUPPRESS`.* Zero breaking change and it does hide the
  visible clutter, but the namespace keeps growing and every future maintenance verb makes it
  worse. Grouping is the structural fix; hiding is cosmetic.
- *Group, but keep the flat names as hidden aliases for a release.* Softer migration, but it
  perpetuates the mess it is meant to remove: two spellings to document, test, and eventually
  delete, and nobody learns the new form until the old one disappears anyway.

`argparse` nests directly: `repo = sub.add_parser("repo"); rsub = repo.add_subparsers(dest="repo_cmd", required=True)`.

## Acceptance criteria
- [ ] `trck repo normalize|renumber|install-hook` work and are listed under `trck repo --help`
- [ ] `trck --help` lists `repo` but not the moved verbs; `init`/`update` still appear there
- [ ] The old flat spellings are **gone** — `trck normalize` exits nonzero with argparse's
      unknown-command error
- [ ] `trck repo` with no sub-verb errors and prints the group's help (`required=True`)
- [ ] The hook written by `install-hook` still invokes `trck check` and keeps working; its header
      comment names the grouped spelling
- [ ] Tests cover each moved verb under its new spelling, and assert the old spelling is rejected
- [ ] README, `CLAUDE_MD_TEMPLATE`, and `issues/CLAUDE.md` verb lists updated

## Notes
Blocks #x2exfdf so `migrate-layout` is born as `trck repo migrate-layout` and never needs
renaming in a released CLI. That is the only hard ordering between this and the layout epic
(#2srvf6j) — the rest of the epic is independent.

Ships in v0.23.0 (#82an2dy) alongside the layout change, so users take one breaking upgrade
rather than two. The release notes must list the moved verbs explicitly — argparse's
"invalid choice" error names the valid choices but won't tell anyone *where* `normalize` went.
