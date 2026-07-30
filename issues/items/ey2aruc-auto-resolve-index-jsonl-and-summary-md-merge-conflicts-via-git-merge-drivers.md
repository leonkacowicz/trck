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
criteria below happened to test.

The `SUMMARY.md` half is unaffected by all of this and remains sound as written.

## The decided design: row-wise 3-way, tuple-atomic, conflict loudly

A merge driver receives three inputs — `%O` (common ancestor), `%A` (ours), `%B` (theirs). The
diff `base → ours` **is** the transaction that produced our side, recovered at field
granularity. So the driver merges *rows keyed by id*, not lines, and per row uses the base to
decide who changed what.

**Field classes.** Not every field merges the same way:

| Class | Fields | Rule |
|---|---|---|
| Set-valued | `labels`, `depends_on` | Union — both sides adding different labels is not a conflict |
| Derived | `status`/`points` on **non-leaves** | Never merge; recompute. `normalize_statuses` derives a parent's status from its children and `normalize_points` its points, so a parent-level divergence is never real |
| Monotone | `created`, `started` | Earliest |
| Scalar | `title`, `slug`, `priority`, `kind`, `spec`, `pr`, `parent` | One side changed → take it; both changed differently → conflict |
| **Lifecycle tuple** | `status`, `closed`, `resolution` (on a leaf) | **Atomic — see below** |

**The lifecycle tuple is atomic, and that is the crux.** `(status, closed, resolution)` is
maintained as a unit by `move_issue`, which clears `closed` and `resolution` on any move to a
non-terminal status. Merging its members independently synthesizes rows no verb can produce —
and it does so **without either side's fields diverging**, so a per-field conflict rule never
fires:

```
base    status=done     closed=T1    resolution=None
ours    trck done #x --resolution wontfix   → status=done (unchanged), resolution=wontfix
theirs  trck mv #x ongoing                  → status=ongoing, closed=None, resolution=None

field-wise:  status → ongoing (only theirs) · resolution → wontfix (only ours) · closed → None

result  status=ongoing  closed=None  resolution=wontfix     ← no verb can produce this
```

So the rule is: **if either side touches any member of the tuple, the tuple merges as a unit —
take one side wholesale, or conflict.** Note this is strictly stronger than "conflict when both
sides change status": above, only *one* side changed status, and the result is still corrupt.
(#nuf3t68 makes that particular state detectable by `check`, independent of merging.)

**And when both sides moved the same leaf: conflict. Loudly.** Rejected in favour of this:

- *Lattice-max over the configured status order* (take the furthest along, on the theory that
  work progresses). It silently discards a **reopen** — a deliberate backward move — and, more
  fundamentally, it is a per-field rule, so it cannot see the tuple problem above at all.
- *Timestamp tiebreak.* Weak: `started`/`closed` only stamp on role boundaries, so
  `ongoing → in-review` stamps nothing, and there is no tiebreak available exactly when needed.

Two people made incompatible claims about the same fact. Better to **not know** than to be
**confidently wrong**.

## Rejected: an append-only operation log

Tempting framing — record the verbs rather than the resulting state, regenerate `index.jsonl`
from the log the way `SUMMARY.md` is regenerated from the index. Two branches never touch the
same line, so `merge=union` becomes textually always-valid.

**It does not solve the problem.** An operation is not a self-contained fact: `done #x` carries
an implicit precondition about the state it expected. Replaying it against a state another
branch changed underneath produces a result nobody authored. The log makes concurrent writes
*representable*, not *meaningful* — it converts a merge conflict into a silent semantic one,
which is strictly worse, because it looks resolved. All the costs of event sourcing (compaction,
deterministic replay ordering, rewriting what `check` validates) buy a false guarantee.

## Do this first: prefer rebase over merge

Cheapest thing that matches the transaction model, and it needs no code. Rebase replays each of
our commits onto theirs one at a time, so a conflict is scoped to a single commit's rows instead
of a whole branch's — which is the transaction framing, literally. Merge drivers apply during
rebase too, so this composes with the driver rather than competing with it. It also matches this
repo's existing preference for linear history.

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
- [ ] Both sides moving the same leaf's status **conflicts** rather than picking a winner.
- [ ] The lifecycle tuple merges atomically: the base/ours/theirs case in the design section
      above conflicts, and specifically does **not** yield
      `status=ongoing, resolution=wontfix`.
- [ ] Independent fields still auto-merge: one side setting `priority` while the other closes
      the issue completes with no conflict.
- [ ] `labels` and `depends_on` union across both sides rather than conflicting.
- [ ] Non-leaf `status`/`points` are recomputed after the merge, never merged.
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

Re-audited 2026-07-30 against the flat-layout change (#2srvf6j); the design section above was
settled in the same pass. Related: #nuf3t68 makes the corrupt lifecycle tuple detectable by
`check` whatever produced it — merge driver, hand-edit, or a botched manual resolution.

Deliberately **not** doing: rejecting a `pr` on a terminal issue. A closed issue keeping its
pull-request link is desirable — it is the review record for the change that resolved it, and
`pr` is a forge-agnostic URL trck never interprets as open or merged.
