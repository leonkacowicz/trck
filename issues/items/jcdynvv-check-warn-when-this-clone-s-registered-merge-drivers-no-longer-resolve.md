# check: warn when this clone's registered merge drivers no longer resolve

## Summary

`trck repo setup-git` bakes the absolute path of the running binary into this clone's
`.git/config`, deliberately — a `PATH` lookup fires much later, from whatever environment git
happens to have, and need not resolve to this engine or any engine at all. The cost of that
choice is that the registration goes stale silently, and nothing ever says so.

When it does go stale the failure is not a clean fallback to an ordinary 3-way merge. Git runs
the driver command, the command does not exist, the exit status is non-zero — so git marks the
path conflicted and leaves `%A` exactly as it found it, which is *ours*. The result is a file
that `git status` calls `UU` and that contains **no conflict markers**: the incoming side's rows
are simply gone. Resolving it the obvious way — "no markers, so nothing to reconcile, `git add`
it" — commits the loss.

Observed twice in this repository during one afternoon, on a clone whose config still read:

```
merge.trck-index.driver = python3 "/…/trck" repo merge-index %O %A %B
python3: can't open file '/…/trck': [Errno 2] No such file or directory
```

That is the retired Python engine, from before the Rust port. Both times a row for a
newly-created issue vanished from `index.jsonl` and had to be rebuilt with
`trck new --id <id>`. `trck repo setup-git` repoints it, but only if you already know that is
what happened.

## Acceptance criteria
- [ ] `trck check` warns when `<tracker>/.gitattributes` names trck's drivers but this clone's
      `.git/config` has no matching driver command — the never-ran-setup-git case, which today
      also merges as plain text with no hint why.
- [ ] It warns when a driver *is* registered but its command does not resolve to an executable.
- [ ] The warning names `trck repo setup-git` as the fix.
- [ ] It is a warning, not an error: a clone that has not run `setup-git` is not a broken
      tracker, and `check` gates commits.
- [ ] Silent when there is no `.git` in reach — the drivers run in contexts where there is no
      repository at all, and `check` runs in CI checkouts that will never merge anything.

## Notes

Deliberately narrow: this asks `check` to notice, not `setup-git` to become self-healing. An
engine that silently rewrites `.git/config` whenever it runs is a worse trade — the path is
per-clone state the user owns, and rewriting it from whichever binary happened to run is how a
tracker ends up pointing at a build artifact someone is about to `cargo clean`.

That last part is worth its own thought: in this repository `setup-git` now points at
`target/release/trck`, which is correct — the engine under change is the one that should answer —
and is also a path that a clean rebuild deletes. The warning covers that case too, which is
most of why it is worth having here rather than only in consumer repos.

Whether the driver could do better on its own is a separate question and probably a "no": when
the command cannot execute, trck is not running, so there is nothing of ours left to fall back
to. Detection ahead of time is the whole of the fix.

Related: #7ed853j (the `setup-git` registration) and #ex5cugg (the driver entrypoints), both
shipped under #ey2aruc.
