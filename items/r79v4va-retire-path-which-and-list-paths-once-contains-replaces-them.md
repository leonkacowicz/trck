# retire path, which and list --paths once --contains replaces them

**Closed `wontfix` on 2026-08-16. They stay.** The reasoning is at the end; the original
proposal is left intact above it, because the argument it lost is the useful part.

## Summary
`path`, `which` and `list --paths` exist to hand a search tool a set of files and read its
output back — a bridge that only spans a tracker made of files. On the `trck-issues` ref
there are none, so all three resolve through `Ctx::dir` and refuse. Once `list --contains`
(#ubvkhds) lands, they have no remaining job here: delete them, along with
`src/query/paths.rs`, rather than leaving three verbs whose whole documented purpose is a
pipeline this repo can no longer run.

The deletion is deliberately **after** the replacement, not with it — do not remove the
escape route before `--contains` exists.

## Why it was rejected

The proposal rested on: *two ways to search bodies, one of which only works in one storage
mode, is worse for everyone than one that works in both.* That holds only if both are
presented as general. Measured after `--contains` landed:

- Against a directory tracker the whole pipeline still round-trips —
  `list --paths --all | which --ids` returns the right ids against `examples/action-game`.
  Nothing is broken for the storage mode these verbs were written for.
- Against a ref they fail cleanly: exit 1, the real cause named, no path printed that is not
  there.

So this is not a broken feature being carried; it is a working feature of a supported storage
mode. Deleting it would take something away from every other repository in order to tidy up
this one — and this repository is the only one that migrated. `--contains` earns its place by
working in both modes, not by leaving nothing else standing.

The issue named this outcome in advance and required it be taken cleanly: keep them
directory-only and close `wontfix`, rather than let the change quietly become half a
deletion. That is what happened.

## What was real in it

One thing: the refusal is accurate but incomplete. It says the tracker has no files; it does
not say that `list --contains` is what the workflow became, and no help text says these three
need a tracker directory. That gap is #ghwwpvk, filed outside the storage epic because it is
about how the engine explains itself to everyone rather than about this repository's move.

The conformance arithmetic in the notes below is therefore moot — no fixtures are deleted and
the floor does not move. Recorded because the reasoning about *when* a floor may come down is
worth keeping: a fixture deleted along with the feature it describes is not a regression,
which is a different case from one that starts failing.

## Acceptance criteria (of the rejected proposal)
- [ ] `src/query/paths.rs` deleted: `cmd_path`, `cmd_which`, `which_operands`, `paths_of`,
      and the Windows `plain`/`needs_verbatim` helpers with their unit tests.
- [ ] `--paths` gone from `ListOpts` and `cmd_list`, including the `--json`/`--paths`
      mutual-exclusion error (src/query/list.rs:155).
- [ ] Dispatch and tables cleaned: src/cli/dispatch.rs:90-94, the `which` row in
      src/cli/mod.rs:151, src/cli/tables.rs:65.
- [ ] Help entries for `path` and `which` removed from src/help/read.rs, and the `--paths`
      option line from `list`'s.
- [ ] An unknown verb is the only thing `trck path` / `trck which` can now produce — no
      stale entry in `trck --help`.
- [ ] Conformance: the eleven fixtures below deleted, and
      `migrate-layout-leaves-a-flat-tracker-usable` rewritten (its `cmd` is
      `list --paths --all`, so it needs another way to show a flat tracker is usable).
- [ ] Docs: CLAUDE.md's *If you need to see the tracker as files* section loses the
      sentence naming all three; skills/trck/SKILL.md:127, :134 and :143 lose their
      references; README's search section is already #ubvkhds's to rewrite.
- [ ] `ratchet generate` re-run and the report staged (a `src/` change).

## Notes
**The conformance floor has to come down, and that needs saying out loud.** Eleven fixtures
go:

    list-json-and-paths-together-are-refused   path-of-an-unknown-id-is-an-error
    list-paths-prints-file-paths               path-prints-one-issues-file
    path-resolves-an-unambiguous-prefix        path-without-an-id-is-a-usage-error
    which-ids-prints-bare-ids                  which-maps-a-body-path-back-to-its-issue
    which-prints-rows-in-tracker-order         which-skips-a-path-that-is-not-an-issue
    which-with-no-issue-paths-prints-nothing

There are 282 fixtures and the committed floor is `--min-pass 280` (.github/workflows/
ci.yml:211). `run.py` returns 1 on `passed < min_pass`, so this change cannot go green
without lowering it — against the rule that the floor only moves up. That rule is about a
fixture that *starts failing*; one deleted along with the feature it describes is not a
regression, and the specification is smaller by exactly as much as the surface is. Lower
the number in the same commit and say why in the comment above it. #ubvkhds lands first and
adds fixtures of its own, so take the arithmetic from the tree, not from 282.

**The one real question to settle first:** directory-backed trackers still exist for other
repos, and for them these three verbs work fine — deleting them removes a working feature
from a supported storage mode. The case for deleting anyway is that two ways to search
bodies, one of which only works in one mode, is worse for everyone than one that works in
both; `--contains` is defined to answer identically from a ref or a directory. If that
argument does not survive contact with the implementation, the fallback is to keep them
directory-only and documented as such — but then this issue closes as `wontfix`, it does
not quietly become half a deletion.
