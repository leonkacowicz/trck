# Auto-resolve index.jsonl and SUMMARY.md merge conflicts via git merge drivers

## Summary
Both tracked tracker files conflict on every branch merge that touches issues:
- `index.jsonl` — both branches append rows at the same spot, so a default merge conflicts.
- `SUMMARY.md` — a generated rollup; any divergence conflicts, but the resolution is trivial:
  throw both sides away and run `trck summary` to regenerate.

Make git handle both hands-free via custom merge drivers declared in `issues/.gitattributes`:
```
index.jsonl  merge=union
SUMMARY.md   merge=trck-summary
```
- `index.jsonl → merge=union` keeps all rows from both sides.
- `SUMMARY.md → trck-summary` is a driver that ignores all three inputs, runs `trck summary`,
  and writes the fresh rollup into git's `%A` slot — so no conflict ever surfaces.

## CORRECTION — the union half is not sound as originally argued

This issue previously claimed:

> `index.jsonl → merge=union` … is correct **once #65 lands** (random ids ⇒ no key clash ⇒ a
> line-union is always valid)

**That reasoning is wrong**, and was wrong before the flat-layout change. Random ids stop two
branches from *inventing* the same id for *new* issues. They do nothing about both branches
**editing the same existing row** — which is the common case, since every `start`/`review`/`done`
rewrites one line. Union keeps both versions, so the index ends up with two rows for one id:

```
{"id": "abc1234", …, "status": "ongoing", …}
{"id": "abc1234", …, "status": "done", …}
```

`trck check` currently reports that as `OK — 2 issues, 0 errors` and `trck list` renders the
issue twice, in two statuses. See #s5585hq, which this now depends on.

**The flat layout (#2srvf6j) made this more dangerous, not less.** Under the old status-folder
layout, both branches moving one issue also moved its body file into two different folders — a
rename/rename conflict git stopped on, forcing a human to look. That was never a designed
safeguard, but it did mean the union hazard could not pass silently. A status change is now a
pure index edit, so nothing in git raises anything, and nothing in `check` catches it either.

**What this means for the design.** `merge=union` is only valid for the append-only case (both
branches ran `trck new`, no shared row touched) — which is exactly the case the acceptance
criteria below happened to test. Before implementing, settle:

- Is union acceptable with #s5585hq turning the bad case into a loud `check` failure the author
  resolves by hand? (Cheapest: automatic for the common case, loud for the rest.)
- Or does `index.jsonl` need a real **custom driver** — one that parses all three inputs and
  merges row-wise by id, taking the non-ancestor side per field, and conflicting only on a
  genuine same-field divergence? (Correct in every case; much more work, and it has to be
  written in something a `.git/config` line can invoke.)

The `SUMMARY.md` half is unaffected by all of this and remains sound as written.

## The thing git makes you do (the crux)
`.gitattributes` is committed and shared, but it can only *name* a driver — it cannot define
the driver's command. The actual `driver = …` shell line lives in `.git/config`, which is
**per-clone and intentionally not shared** (otherwise cloning a repo would be remote code
execution). So the driver does nothing until each clone registers it locally. Automating that
registration is the real work here — and trck already has the seam: `trck init` installs it
for consumer repos; this self-hosting repo needs a one-time setup (a `trck setup-git` verb, or
fold it into `init`/`update`).

## Acceptance criteria
- [ ] `issues/.gitattributes` declares `index.jsonl merge=union` and `SUMMARY.md merge=trck-summary`.
- [ ] A `trck-summary` merge driver regenerates SUMMARY into `%A` (ignores `%O`/`%A`/`%B`
      contents) and exits 0.
- [ ] Driver command is installed into `.git/config` automatically — by `trck init` for
      consumer repos, and via a documented one-time setup for this repo.
- [ ] A real two-branch merge (each branch ran `trck new`) completes with **zero manual
      conflict resolution**, and `trck check` passes on the result.
- [ ] **The case that actually breaks:** a two-branch merge where *both branches mutate the same
      issue* (e.g. one runs `trck start #x`, the other `trck done #x`) ends in a state a human
      can recover — either a loud `trck check` failure naming the duplicated id (#s5585hq), or a
      correct row-wise merge. **Silently accepting two rows for one id is a failing outcome.**
- [ ] Tests/fixtures exercising the union of index rows and the SUMMARY regeneration path.
- [ ] Tests/fixtures for the same-issue-both-sides merge, asserting the chosen behaviour.

## Notes
Ordering subtlety avoided by design: a driver that regenerates SUMMARY *from* index.jsonl
assumes the index is already merged, but git gives no ordering guarantee between per-file
driver runs. Making `index.jsonl` conflict-free via `merge=union` sidesteps this — the union
result is stable regardless of when SUMMARY's driver fires. (This is why #066 leans on #65.)

Rejected alternative: `SUMMARY.md merge=ours` + a `post-merge` hook running `trck summary`.
It works but leaves SUMMARY dirty *after* the merge commit, forcing a follow-up commit — worse
ergonomics than the driver, which resolves inline.

Originally noted as depending on #65 (random ids) for the `index.jsonl merge=union` half to be
sound. That dependency was necessary but **not sufficient** — see the correction above. Now
depends on #s5585hq, which makes the failure detectable at all. Tagged `conflict-resolution`
alongside #64/#65.

Re-audited 2026-07-30 against the flat-layout change (#2srvf6j).
