---
name: trck-worktree
description: Use when about to run a trck verb that writes (new, start, review, done, set, dep, label, mv) in a repo whose tracker is committed and whose main branch is protected — filing an issue found mid-task, closing one, re-prioritising, adding a dependency — and especially when the working tree is dirty or sitting on a feature branch. Also use when a tracker-only change would otherwise need its own branch, PR and merge.
---

# trck in a throwaway worktree

Tracker writes go to `main` directly, from an isolated worktree, never from your working branch.

**Core principle:** an issue row is not code. It cannot break the build, so it does not need a
branch, a PR, or a review — it needs to land on `main` without disturbing whatever you were doing.
A detached worktree at `origin/main` gives you that, and keeps tracker commits out of your feature
diff.

## When to use

**Use for every write verb:** `new`, `start`, `review`, `done`, `set`, `dep`, `label`, `mv`.

**Do NOT use for read verbs.** `list`, `tree`, `ready`, `next`, `deps`, `show`, `path`, `check`,
`summary`, `diff` read the tracker in your current checkout. Run them normally — the ritual buys
nothing and costs a fetch.

## Preconditions

- `trck repo setup-git` has been run in this clone. `.gitattributes` is committed and declares
  `merge=trck-index` / `merge=trck-summary`, but the driver *commands* are per-clone. Without them
  a contended rebase does not conflict, it dies: `fatal: custom merge driver trck-index lacks
  command line.`
- You can push to `main` directly (admin bypass, or `main` unprotected). If not, stop and say so
  rather than inventing a PR flow.

## The ritual

```bash
# The suffix must be unique per session and stable across your own tool calls: use the basename
# of your scratchpad directory, or a token you generate once and reuse for the rest of the turn.
# NOT $$ — each shell invocation is a new process.
WT="$(git rev-parse --git-common-dir)/trck-wt-$SESSION"   # inside .git — invisible to trck discovery
git fetch origin main
git worktree add --detach "$WT" origin/main

# ALWAYS --dir. Without it trck walks up and writes to your feature branch.
# Write the body first, then hand it over: with no body flag and no terminal, `new` refuses
# rather than filing prose nobody wrote. Use `--empty` for a deliberately title-only issue.
printf '%s\n' '# title' '' '## Summary' '…' > "$WT/body.md"
path=$(trck --dir "$WT/issues" new "title" --priority high --body-file "$WT/body.md")
```

`--body TEXT` and `--body-file -` (stdin) work too, and are the same three spellings `git commit`
uses. Metadata verbs (`done`, `set`, `dep`, …) have no body step; go straight on.

```bash
trck --dir "$WT/issues" check          # gate before pushing; there is no PR to catch this

git -C "$WT" add -A
git -C "$WT" commit -m "file: <title>"

for _ in 1 2 3 4 5; do
    git -C "$WT" push origin HEAD:main && break
    git -C "$WT" fetch origin main
    git -C "$WT" rebase origin/main    # drivers merge index.jsonl, regenerate SUMMARY.md
done

git worktree remove --force "$WT"      # yours, identified by $SESSION — never anyone else's
```

The push loop is optimistic concurrency: a rejection means `main` moved, the rebase replays your
one commit on top, and the drivers resolve `index.jsonl` and `SUMMARY.md` without asking. Verified
against a contended push; it converges on the second attempt.

## Concurrent sessions

Several agents may be filing at once in the same clone. The per-session `$SESSION` suffix is what
makes that safe: each gets its own worktree, and the push loop already handles two of them landing
on `main` at the same time — the second rebases, the drivers resolve, both commits survive.

**Never remove a worktree you did not create.** A path that already exists is another live session
holding it, not debris — and force-removing it destroys an issue body that agent is still writing,
mid-turn, with no error until its next `git add` fails on a vanished path. If your own
`worktree add` ever fails on an existing path, your `$SESSION` token is not unique; fix that rather
than clearing the path.

## Quick reference

| Situation | Do |
|---|---|
| Dirty tree, mid-feature, need to file a bug | Full ritual. Your branch is never touched. |
| `trck ready` / `next` / `list` | Run directly, no worktree. |
| `$WT` already exists | Another session, or your own abandoned turn. Never force-remove it — pick a unique `$SESSION`. |
| Cleaning up genuinely dead worktrees | `git worktree prune` (only removes ones whose directory is already gone). |
| Rebase conflicts in `items/*.md` | Two edits to one body. Resolve by hand, `git rebase --continue`. |
| `fatal: custom merge driver … lacks command line` | `trck repo setup-git`, then retry. |
| Push rejected 5 times | `main` is busier than this loop. Report it; don't force-push. |

## Common mistakes

- **Bare `trck new` from the repo root.** Discovery walks up to the nearest `trck.json`, finds your
  primary checkout, and writes the row onto your feature branch — exactly the coupling this avoids.
  `--dir "$WT/issues"` on every invocation is the guard.
- **`git pull` instead of `fetch` + `rebase`.** Creates a merge commit on `main`.
- **`push --force`.** The rejection means someone else landed work. Rebase, never overwrite.
- **Skipping `trck check`.** With no PR there is no pre-merge gate; a bad row lands on `main` and
  turns CI red after the fact.
- **Leaving the worktree behind.** With per-session paths it no longer blocks the next turn, so
  nothing tells you — they just accumulate inside `.git`. Remove yours at the end of the turn.
- **Reusing a `$SESSION` token across sessions.** Collapses back to one shared path, which is the
  collision this design exists to avoid.
