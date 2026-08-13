# storage: move the tracker to an orphan trck-issues branch

## Summary
The tracker stops living in `main`'s tree and moves to its own orphan branch, `trck-issues`,
whose root *is* the tracker (`trck.json`, `index.jsonl`, `items/`, `SUMMARY.md` at top level).
Reads resolve from the git ref instead of the working tree; writes go through git plumbing
instead of a checkout. `skills/trck-worktree/SKILL.md` is deleted, because the ritual it
documents becomes the engine's business rather than the operator's.

**The two things this fixes.**

1. *Coupling.* Tracker commits can no longer appear in a feature diff, and a write no longer
   rebases over unrelated code that happened to land on `main`. This is what the worktree
   ritual buys today, at the cost of six git commands per issue filed.
2. *Time-travel on reads.* Today `list`/`next`/`ready`/`html` read the tracker in the current
   checkout, so on a feature branch you plan against the tracker as of the branch point.
   Reading from a ref is branch-independent by construction — the bug goes away structurally
   rather than by discipline.

**The unlock.** `--dir` is load-bearing today only because a bare `trck new` walks up, finds
the primary checkout, and writes the row onto your feature branch. Once nothing tracker-shaped
is in the working tree, a bare `trck new` has exactly one thing it can mean. The footgun
disappears, and with it the reason the operator ever sees a worktree.

## Design

### Branch name — convention, not configuration

`trck-issues`, discovered by asking git whether the ref resolves. **No marker file on `main`:**
`git rev-parse --verify --quiet origin/trck-issues` is enough, so `main` keeps zero tracker
artifacts, `trck.json` gains no new key, and `check` gains no new warning.

Not `trck/issues`: git cannot hold both `refs/heads/trck` and `refs/heads/trck/issues`
(directory/file conflict), and `trck` is a branch name this repo would plausibly want.

Escape hatch for a name collision, a monorepo with two trackers, and tests: `--ref REF` and
`$TRCK_REF`, slotted into the existing resolution order in `discovery.rs`.

### Discovery precedence

`--dir` → `$TRCK_DIR` → `--ref`/`$TRCK_REF` → working-tree walk-up → `origin/trck-issues` →
local `trck-issues`.

**A working-tree tracker beats the ref.** That keeps `trck init` unsurprising, and it makes the
migration safe in stages: nothing changes behaviour until `issues/` is actually removed from
`main`.

### Reads

`git show <ref>:index.jsonl` for the index, `<ref>:items/<id>-<slug>.md` for a body,
`<ref>:trck.json` for the format version. No worktree, no network, no fetch.

The plumbing already exists — `diff.rs::git_snapshot` and `git_tracker_prefix` do exactly this
today for `trck diff`. The tracker prefix becomes empty, since the tracker is at the branch root.

**Freshness.** Reads must not auto-fetch: too slow, and it makes every read need the network.
But a `trck next` planning against a week-old `origin/trck-issues` is the time-travel bug in a
new costume. Reads should surface the ref's age when it is beyond some threshold, so staleness
is visible rather than silent.

### Writes — plumbing, no worktree

```
GIT_INDEX_FILE=$tmp git read-tree origin/trck-issues
                     git hash-object -w <changed blobs>
                     git update-index --add --cacheinfo ...
                     git write-tree → git commit-tree → git push origin HEAD:trck-issues
```

Works from a dirty tree, on any branch, with nothing to check out and nothing to clean up.

On push rejection the loop **re-executes the operation** against the new ref rather than merging
two written files. Consequence worth noting: the write path never invokes `trck-index`, so the
merge drivers become vestigial — needed only if a human manually merges two divergent copies of
the branch, which should approach never.

**Offline writes still work**, which the current design does not manage: a write advances the
*local* `trck-issues` ref and pushes opportunistically. A failed push leaves a local commit and a
warning, not a lost issue.

### Body input — `new` and `edit`

Follow git's model rather than inventing three flags:

| invocation | behaviour |
|---|---|
| `--body TEXT` | inline, like `git commit -m` |
| `--body-file PATH` | file, like `-F`; **`-` means stdin**, so stdin is not a separate mode |
| `--empty` | deliberate title-only issue, no body |
| no flag, TTY | `$EDITOR` on the prose template |
| no flag, no TTY | **error naming the flags** — never hang on an editor that will not open |

That last row matters for agents and CI: `trck new "title"` non-interactively must fail loudly
rather than block or silently file an empty body.

**`$EDITOR` behaves like `visudo`:**

- edit a temp copy, then validate before accepting — the body-level checks in `validate/`
  (title heading matches, slug sanity);
- on validation failure, re-open with the error at the top rather than discarding the work;
- an empty or unmodified buffer **aborts**: no issue filed, no commit, nothing pushed. Same as
  `git commit`. This is why a deliberate title-only issue needs the explicit `--empty`.
- visudo's *lock* does not transfer — contention is on a remote ref, and the push loop's retry
  is the equivalent, with no stale lock to clean up.

### `trck edit <id>` — required, not a nicety

Today an issue body is edited by opening `issues/items/<id>-<slug>.md`. With the tracker off the
working tree that file does not exist locally, so without this verb body edits become impossible
rather than merely different. Mechanics are identical to `new`'s editor path — fetch body, temp
file, `$EDITOR`, validate, commit, push — so it is the same code.

### Migration

```
git subtree split -P issues -b trck-issues   # full history of issues/, rewritten to the root
git push origin trck-issues
git rm -r issues/                            # on main, as its own commit
```

History is preserved, so `trck diff` over past revisions keeps working on the new branch.

### Fallout to handle

- **Pre-commit hook.** `trck check` on `main` would be checking a ref the commit cannot affect.
  Drop it there; the write path's own validation is the real gate.
- **CI.** `trck check` moves to its own workflow gated on `push: branches: [trck-issues]`. Code CI
  stops knowing the tracker exists, and `scripts/ci_changed.py` loses its `issues/` case entirely —
  a code PR structurally cannot contain tracker changes any more. Update
  `scripts/tests/test_ci_changed.py` first, per the allowlist rule.
- **`trck diff` revision resolution.** `HEAD~5..HEAD` on `main` becomes meaningless; revisions have
  to resolve on the tracker branch. Overlaps with #wtmfdhr.
- **`SUMMARY.md` leaves the repo front page.** Link to `blob/trck-issues/SUMMARY.md` from the
  README.
- **Docs.** `CLAUDE.md` (both root and `issues/`) describe the worktree ritual at length; all of
  it goes.

### The constraint this respects

**A tracker stays "a directory".** The ref is a *source* for one, resolved in a thin layer above
the existing model — not a replacement for it. That is what keeps `conformance/run.py` intact:
it still execs the binary against a plain directory in a temp dir, with no git anywhere. Any
design that made git a hard dependency of the data model would have cost the executable spec,
which is the reason a hosted backend was ruled out.

## Acceptance criteria
- [ ] `discovery.rs` resolves a tracker from a git ref when no working-tree tracker is found;
      order is `--dir` → `$TRCK_DIR` → `--ref`/`$TRCK_REF` → walk-up → `origin/trck-issues` →
      `trck-issues`, and a working-tree tracker wins over the ref.
- [ ] Every read verb (`list`, `tree`, `ready`, `next`, `deps`, `show`, `check`, `summary`,
      `html`) works from any branch with a dirty tree and no `issues/` directory.
- [ ] Reads never fetch; a ref older than a threshold is reported rather than silently used.
- [ ] Write verbs (`new`, `start`, `review`, `done`, `set`, `dep`, `label`, `mv`) commit and push
      to the ref via plumbing, with no worktree and no checkout, from a dirty tree on any branch.
- [ ] Push rejection retries by re-executing the operation against the refetched ref; converges
      under a contended push (test as `ey2aruc`/`broken_pipe` do — two writers, one clone).
- [ ] A failed push leaves the local ref advanced and warns; the issue is not lost.
- [ ] `new` accepts `--body TEXT`, `--body-file PATH` (`-` = stdin), and `--empty`; with none of
      them it opens `$EDITOR` on a TTY and errors naming the flags without one.
- [ ] `$EDITOR` path validates on save, re-opens with the error on failure, and aborts on an
      empty or unmodified buffer without writing anything.
- [ ] `trck edit <id>` edits an existing body through the same path.
- [ ] `--ref`/`$TRCK_REF` override the conventional name.
- [ ] `git subtree split` migration documented and performed; `issues/` removed from `main` in its
      own commit; `trck diff` still resolves pre-migration revisions.
- [ ] `trck check` runs in a workflow gated on `push: branches: [trck-issues]`; `ci_changed.py`
      drops its `issues/` case, with `scripts/tests/test_ci_changed.py` updated first.
- [ ] Pre-commit hook no longer runs `trck check` on `main`.
- [ ] `skills/trck-worktree/SKILL.md` deleted; both `CLAUDE.md` files updated.
- [ ] Conformance fixtures still exec against plain directories — the ref layer is tested
      separately and the suite's method is unchanged.

## Notes

### What this does NOT solve

**Concurrent sessions still contend on `index.jsonl`.** Two agents filing at once still write the
same file, and the outcome still depends on either the `trck-index` merge driver or — on the
plumbing write path — the re-execute retry. The orphan branch fixes *coupling*, not *contention*.

The two are independent and can be decided separately. The eventual fix for contention is the
**per-issue metadata layout**: `items/<id>.md` (prose, hand-edited) beside `items/<id>.json`
(engine-owned), so concurrent writes to different issues touch disjoint paths and git merges them
with no driver at all. Analysis from the design discussion:

| case | per-file | unified + driver |
|---|---|---|
| two new issues | git, no driver | driver, correct |
| edits to different issues | git, no driver | driver, correct |
| same issue, different fields | **conflicts** (one object, one line) | driver merges cleanly |
| same issue, same field, divergent | conflicts | conflicts |

So per-file does not strictly dominate — it inverts on row three, and would need its own driver
on `items/*.json` to match. The real asymmetry is *preconditions*, not outcomes: per-file needs
no driver for the common cases, unified always does. Which means the trigger for splitting is the
driver being **absent** (`fatal: custom merge driver trck-index lacks command line` in a fresh
clone, CI runner or sandbox that never ran `repo setup-git`), not the driver being wrong — it is
396 tested, symmetric lines and it is not going to start producing garbage.

**Cheaper mitigation to do first:** have a write verb detect the missing-driver failure, run
`trck repo setup-git`, and retry. A handful of lines, and it removes the whole failure class.
Note the plumbing write path sidesteps drivers entirely anyway, which may make this moot.

The strongest reason to split later has nothing to do with conflicts: per-file makes
`git log items/<id>.json` the issue's real status history, which answers #gybeetp decisively in
favour of git reconstruction and makes `list --as-of` (#xr994r6) and time-in-status (#ut9bqm4)
nearly free.

### Alternatives considered and rejected

- **Custom ref namespace (`refs/trck/issues`), git-notes style.** Same objects, same push CAS,
  same everything — the only difference is the refname. But `refs/*` outside `refs/heads/` is not
  in the default fetch refspec, so `git clone` gets nothing and a missing ref reads as an *empty
  tracker* rather than an error. `actions/checkout` needs a manual refspec. This is the trap that
  has dogged `git notes` for fifteen years. The namespace is right for machine-generated
  annotations nobody browses; wrong for a tracker humans and CI read.
- **Issues branch merged back into `main` with a merge commit.** Cancels its own benefit — `main`
  ends up with every tracker commit anyway, plus a merge commit per cycle — and needs an invariant
  that `main` never touches `issues/` or the aging merge base starts conflicting.
- **A daemon owning a checkout (`trck serve`, #tcm5s56).** Solves contention properly via a single
  writer, but too many moving parts for a tool whose point is that there is nothing to run.
- **Hosted API backend (Postgres, thin-client CLI).** Crosses from "local files" to "network
  service" — the axis a local fix cannot reach, and the one that buys phone filing, no-checkout
  access and cross-machine sessions. Ruled out here on three grounds: the standard library has no
  HTTP client and no TLS, so it is incompatible with **no dependencies, ever** rather than merely
  in tension with it; `conformance/run.py`'s exec-and-diff method does not survive a network
  store; and backups, offline access and zero ops are properties currently obtained for free from
  git that a personal service would have to re-earn.

### Open questions

- Threshold and presentation for the stale-ref warning on reads.
- Does `trck init` learn to create the branch, or is that `trck repo init-branch`?
- Does anything still want a real checkout of `trck-issues` (bulk body editing, grep across
  bodies), or is `edit` plus read verbs sufficient?
