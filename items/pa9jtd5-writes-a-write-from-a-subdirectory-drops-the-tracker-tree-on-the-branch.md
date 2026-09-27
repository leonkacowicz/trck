## Summary

On a branch-mode tracker (the tracker is the root of the `trck-issues` ref), any write verb
run from a **subdirectory** of the working tree commits a tree that holds only `index.jsonl`
and `SUMMARY.md`. Everything else on the branch — `trck.json`, every file under `items/`,
the scaffolded `CLAUDE.md` / `README.md` — is deleted in that commit, and the write verb then
pushes it. The next write fails with:

    error: git ref 'trck-issues' holds no trck.json, so it is not a tracker

The same verbs run from the repository root work correctly. Silent data loss that is pushed
to the remote by default.

## Reproduction (trck 0.30.1)

```bash
git init -q -b main demo && cd demo
git commit -q --allow-empty -m init && mkdir sub
git worktree add -q --orphan -b trck-issues ../demo-wt
(cd ../demo-wt && trck init . && git add -A && git commit -q -m tracker)
git worktree remove ../demo-wt

trck new "one" --empty && trck new "two" --empty
git ls-tree -r --name-only trck-issues   # CLAUDE.md README.md SUMMARY.md index.jsonl items/… trck.json

(cd sub && trck start <id-of-one>)        # exits 0
git ls-tree -r --name-only trck-issues   # SUMMARY.md index.jsonl   <- everything else gone
(cd sub && trck new "three" --empty)      # error: … holds no trck.json …
```

Observed with `mv`/`done` and `start` from a subdirectory; `new` from the root before that
was fine.

## Likely cause (unverified)

The commit tree for the write seems to be built from only the paths the verb rewrote,
resolved against the current directory rather than the repository root, so the unchanged
entries of the base tree are not carried over when the working directory is not the root.

## Recovery

The previous commit on the branch is intact, so re-commit its tree on top without rewriting
history, check, and push:

```bash
new=$(git commit-tree <last-good>^{tree} -p <bad> -m "Restore tracker files")
git update-ref refs/heads/trck-issues "$new" <bad>
trck check && trck sync
```

## Acceptance criteria

- A write verb run from any subdirectory produces the same commit as from the root: the
  full base tree plus the verb's changes.
- A regression test covers a write from a subdirectory on a branch-mode tracker.
- Consider a guard: refuse to commit a tracker tree that has no `trck.json`.
