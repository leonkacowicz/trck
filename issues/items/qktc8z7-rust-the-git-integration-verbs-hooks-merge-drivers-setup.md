# rust: the git-integration verbs — hooks, merge drivers, setup

## Summary
Split out of `eek4hat`, which bundled them with `check`. They are a different kind of thing:
`check` reads a tracker and reports, while these shell out to git, write into `.git/config`, and
— in the merge drivers' case — **run inside a merge**, where the working tree is not yet the
merged result.

That last point is why they deserve their own issue. `merge-index`'s contract is not "produces
the right file" but "produces the right file *given three inputs git hands it mid-operation*",
and its behaviour under conflict is part of that contract. Testing it means driving real merges,
which is a harness the rest of the port does not need.

- `repo install-hook` — write a pre-commit hook running `trck check`.
- `repo setup-git` — write `.gitattributes` and register the drivers in *this clone's*
  `.git/config`. Git shares the attributes file but never the driver commands, because that
  would make cloning remote code execution — so this is per-clone, and every clone must run it.
- `repo merge-index` / `merge-summary` — the drivers git invokes. Row-wise, keyed on id.
- `repo migrate-layout` — the pre-0.23 status-folder migration, and whatever `rbast9r` lands.

## Acceptance criteria
- [x] `install-hook` and `setup-git`, writing the same files as the Python engine.
- [x] `merge-index` merging row-wise on id, with the same conflict behaviour — including the
      `(status, closed, resolution)` tuple resolved as one unit, which is exactly what a
      field-wise merge gets wrong and what `check` then reports.
- [x] `merge-summary`, which regenerates rather than merges.
- [x] `migrate-layout`, and the refusal every verb gives an unmigrated tracker.
- [x] Tests that drive a real git merge, not a simulated one. The failure mode this code exists
      to prevent only appears when git is the one calling.

## Notes
`repo renumber` is **not** in scope: integer ids were dropped outright (`dfe48ds`) and the
converter lives in `scripts/renumber.py`, deliberately outside the engine.

## Outcome
The conformance suite is now **green on the Rust engine: 225/225**. The three merge fixtures
written ahead of the port in #av3efth were the last red ones.

`repo normalize` came along too — not listed in the criteria, but it is a `repo` verb and the
surface would have been oddly incomplete without it.

**The Rust engine had no legacy-layout guard at all.** The criterion asked for "the refusal every
verb gives an unmigrated tracker" and there was none: the port would have read *and written to* a
pre-0.23 tracker, with status encoded both in the path and in the index. The refusal now lives in
`Ctx::load`, beside the format guard and for the same reason — every verb builds a `Ctx`, so no
call site is left to forget it.

**One deliberate divergence from the reference.** The driver command baked into `.git/config` is
this binary's absolute path with no interpreter prefix, and there is no vendored-copy case: a
vendored `trck` beside a tracker is a Python script, which this engine cannot claim to be. Still
never a bare `trck` — the command fires much later from whatever environment git has, where a PATH
lookup need not resolve and, where it does, need not be this engine. `.gitattributes` is
byte-identical to the Python engine's.

**Symmetry is asserted, not assumed.** Every merge unit test runs both ways round. `git merge` and
`git rebase` from the same branch hand `%A`/`%B` over in opposite order, so an asymmetric rule
fails silently and only for whoever integrated the other way — there is no way to notice it from
one direction.
