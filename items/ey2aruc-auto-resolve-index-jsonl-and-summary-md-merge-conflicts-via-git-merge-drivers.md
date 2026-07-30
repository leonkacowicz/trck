# Auto-resolve index.jsonl and SUMMARY.md merge conflicts via git merge drivers

## Summary
Both tracked tracker files conflict on every branch merge that touches issues:
- `index.jsonl` — both branches append rows at the same spot, so a default merge conflicts.
- `SUMMARY.md` — a generated rollup; any divergence conflicts, but the resolution is trivial:
  throw both sides away and run `trck summary` to regenerate.

Make git handle both hands-free via custom merge drivers declared in `issues/.gitattributes`:
```
index.jsonl  merge=trck-index
SUMMARY.md   merge=trck-summary
```
- `index.jsonl → trck-index` is a **row-wise 3-way merge keyed by id** — see the design section
  below. (An earlier draft said `merge=union`; that is wrong and the correction is recorded
  below.)
- `SUMMARY.md → trck-summary` resolves a file that is 100% derived, so no conflict need ever
  surface — but *when* it can regenerate is constrained; see the ordering problem below.

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

## Rebase is already the workflow — the driver is the only remaining lever

An earlier draft of this issue recommended "prefer rebase over merge" as a cheap first step.
**That advice is already implemented**: this repo has 0 merge commits across 316, and linear
history is the standing preference. There is no switch left to flip.

Recording why rebase helps at all, since it shapes what the driver still has to do: rebase
replays our commits onto theirs one at a time, so a conflict is scoped to a single commit's rows
instead of a whole branch's divergence. Because `index.jsonl` is id-sorted and one commit
touches few rows, the two sides frequently do not overlap textually and no conflict arises.

So the conflicts that actually reach a human here are the **residual** ones rebase cannot
remove: two commits genuinely touching the same rows. Those are exactly what the driver targets.
The driver is not an alternative to the workflow — with rebase already in place, it is the only
thing left that can help.

### It must work under both rebase and merge

Non-negotiable, and easy to get wrong by only ever testing one.

Both paths run the same merge machinery, so a registered driver fires for either. **Verified**
with a probe driver that printed its three operands (git 2.x, default backend), across all three
ways of integrating two branches:

```
(1) on main,    git merge feature   →  O=BASE  A=MAIN     B=FEATURE
(2) on feature, git rebase main     →  O=BASE  A=MAIN     B=FEATURE
(3) on feature, git merge main      →  O=BASE  A=FEATURE  B=MAIN
```

**(2) and (3) are opposite** — same branch, same user intent ("integrate main into my work"),
inverted operands. Meanwhile (1) and (2) *agree*, despite being different operations on
different branches.

So the variable is **not** merge-vs-rebase. `%A` is simply whatever is checked out at that
moment in the operation — the side being merged *into*. In a merge that is your branch; in a
rebase the sequencer checks out the upstream first and replays your commits onto it, so it is
the upstream. Rebase only looks like the odd one out if you compare it against the wrong merge
direction, which is the mistake an earlier draft of this section made.

**The hard consequence: a driver cannot determine which side is "mine".** Not unreliably — not
at all. No operand, and no combination of operands, answers it. `GIT_REFLOG_ACTION` sometimes
hints at the operation, but it still cannot separate (2) from (3) without also knowing the
branch topology, and environment sniffing is not a foundation to build a correctness rule on.

That turns "prefer symmetric rules" from a design preference into a **hard constraint**: any
rule needing to know whose change is whose is *unimplementable* in a merge driver. What remains
available is the `%O` base — who changed what relative to the ancestor — and it is enough,
because the conflict trigger we chose ("both sides changed this field differently") is itself
symmetric. Union of `labels`, earliest timestamp, recomputed derived fields, and the
lifecycle-tuple rule are all already orientation-independent by construction. Keep them that way.

For conflict *messages*, never write ours/theirs/yours — the words mean opposite things across
(2) and (3), so a message using them is wrong half the time. Name what each side contains
instead: `#x: 'ongoing' on one side, 'done' on the other`.

## The ordering problem, reintroduced

The original design sidestepped this and the note at the bottom still records why: a driver that
regenerates `SUMMARY.md` *from* `index.jsonl` assumes the index is already merged, but **git
gives no ordering guarantee between per-file driver runs**. `merge=union` dodged it because a
union result is stable whenever it happens.

**Abandoning union brings the problem back.** If `trck-summary` fires before `trck-index`, it
regenerates the rollup from a pre-merge index and writes a confidently wrong `SUMMARY.md` — the
exact failure class this whole design is built to avoid.

**Resolution: the index driver owns SUMMARY regeneration.** `trck-index` writes `SUMMARY.md`
itself, as a side effect, after producing a clean merged index. Then order genuinely does not
matter:

- `trck-summary` first → it regenerates from a stale index, and `trck-index` overwrites it.
- `trck-index` first → `trck-summary` regenerates from the merged index; same result.

`trck-summary` becomes trivial: regenerate from whatever the index currently says and exit 0. It
is a safety net, not the authority.

**On a conflicted index merge, neither driver touches SUMMARY.** Regenerating a rollup from a
half-merged index would launder a conflict into a plausible-looking file. Leave the previous
`SUMMARY.md` in place, let the human resolve `index.jsonl`, and let any `trck` verb — or the
pre-commit hook — regenerate it. A stale rollup is obvious and harmless; a fabricated one is not.

## The thing git makes you do (the crux)
`.gitattributes` is committed and shared, but it can only *name* a driver — it cannot define
the driver's command. The actual `driver = …` shell line lives in `.git/config`, which is
**per-clone and intentionally not shared** (otherwise cloning a repo would be remote code
execution). So the driver does nothing until each clone registers it locally.

**Verified: both unregistered states are safe, but they differ.** There are two, and the
distinction only surfaced when the integration tests exercised it:

| `.git/config` state | git's behaviour |
|---|---|
| Driver **never defined** — a fresh clone that has not run `setup-git` | Falls back to the built-in 3-way merge and writes normal `<<<<<<<` markers |
| `merge.<name>.name` set but **`.driver` missing** — a partially edited config, or a future rename | `fatal: custom merge driver <name> lacks command line`; the operation aborts and the working tree is left untouched |

The first is the case that matters for rollout: an un-set-up clone is exactly as well off as
today, so registration needs no flag day. The second is louder but equally safe — git refuses
rather than guessing. **Neither silently picks a side**, which is the property being relied on.
Both are pinned by tests.

Automating that registration is the real work here — and trck already has the seam: `trck init` installs it
for consumer repos; this self-hosting repo needs a one-time setup (a `trck setup-git` verb, or
fold it into `init`/`update`).

## Acceptance criteria
- [ ] `issues/.gitattributes` declares `index.jsonl merge=trck-index` and
      `SUMMARY.md merge=trck-summary`.
- [ ] `SUMMARY.md` is correct after a clean merge regardless of the order git runs the two
      drivers in (see the ordering problem above).
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
- [ ] **Every scenario above is exercised in all three integration directions** — `git merge`
      from each side, and `git rebase` — with identical outcomes. Two directions is not enough:
      merging feature→main and rebasing feature onto main happen to agree, so a driver that is
      orientation-dependent passes both and still breaks on `git merge main` from the feature
      branch.
- [ ] No rule anywhere in the driver branches on which operand is "ours". Symmetric, or derived
      from `%O`, only.
- [ ] A conflict message never says "ours"/"theirs"/"yours" — it names what each side contains,
      so it reads correctly whichever direction produced it.
- [ ] With the driver named in `.gitattributes` but **not** registered in `.git/config`, git
      falls back to the ordinary 3-way merge and leaves normal conflict markers — verified, so
      an unregistered clone is no worse off than today (no silent one-sided resolution).

## Notes
~~Ordering subtlety avoided by design: … making `index.jsonl` conflict-free via `merge=union`
sidesteps this.~~ **Superseded.** Dropping union brought the ordering problem back; it is now
handled explicitly by making the index driver own SUMMARY regeneration — see "The ordering
problem, reintroduced" above.

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
