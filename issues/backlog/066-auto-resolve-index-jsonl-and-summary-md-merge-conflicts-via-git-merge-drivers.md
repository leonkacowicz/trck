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
- `index.jsonl → merge=union` keeps all rows from both sides. This is correct **once #65
  lands** (random ids ⇒ no key clash ⇒ a line-union is always valid; `trck normalize`/`check`
  tidies order).
- `SUMMARY.md → trck-summary` is a driver that ignores all three inputs, runs `trck summary`,
  and writes the fresh rollup into git's `%A` slot — so no conflict ever surfaces.

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
- [ ] Tests/fixtures exercising the union of index rows and the SUMMARY regeneration path.

## Notes
Ordering subtlety avoided by design: a driver that regenerates SUMMARY *from* index.jsonl
assumes the index is already merged, but git gives no ordering guarantee between per-file
driver runs. Making `index.jsonl` conflict-free via `merge=union` sidesteps this — the union
result is stable regardless of when SUMMARY's driver fires. (This is why #066 leans on #65.)

Rejected alternative: `SUMMARY.md merge=ours` + a `post-merge` hook running `trck summary`.
It works but leaves SUMMARY dirty *after* the merge commit, forcing a follow-up commit — worse
ergonomics than the driver, which resolves inline.

Depends on #65 for the `index.jsonl merge=union` half to be sound. Tagged `conflict-resolution`
alongside #64/#65.
