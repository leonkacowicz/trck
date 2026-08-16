# migration: split issues/ onto an orphan trck-issues branch, history intact

## Summary
```
git subtree split -P issues -b trck-issues   # full history of issues/, rewritten to the root
git push origin trck-issues
```

Rewriting to the root is what makes `<ref>:index.jsonl` work without a prefix. History is
preserved, so `trck diff` over past revisions keeps working.

Publishing the branch changes nothing on its own: while `issues/` is still in `main`'s tree, the
working-tree tracker wins (#B5). That is what makes this reversible and the flip (#E21)
separate.

## Acceptance criteria
- [x] `origin/trck-issues` exists, its root is the tracker, and `trck --ref` reads it correctly.
- [x] The split is dry-run and diffed against the working-tree tracker before pushing: same index, same bodies, same summary.
- [x] `trck diff` resolves a pre-migration revision on the new branch (#E17).
- [x] `main` is unchanged by this task.
- [x] The exact commands run are recorded in the body, so the operation is auditable.

## Notes
One-shot and manual, but the verification is mechanical — the two trees must match.

## What was run (2026-08-16)

Base: `main` at `77d9aa4`, working tree clean, nothing else in flight in this clone.

```
git subtree split -P issues -b trck-issues     # -> 565c185, 330 commits
git diff --stat HEAD:issues trck-issues        # -> empty
git rev-parse HEAD:issues trck-issues^{tree}   # -> 8bf9806… twice
trck --ref trck-issues check                   # -> OK — 287 issues, 0 errors, 0 warnings
trck --ref trck-issues diff HEAD~40            # -> resolves, reports the expected deltas
git push origin trck-issues                    # -> [new branch]
trck --ref origin/trck-issues check            # -> OK — 287 issues, 0 errors, 0 warnings
```

**The tree equality is an object-id equality, not a diff that came back empty.** `HEAD:issues`
and `trck-issues^{tree}` are the same object, `8bf9806`, so index, bodies, summary,
`.gitattributes` and `trck.json` are byte-identical by construction — there is no file the
comparison could have skipped.

History survived whole: 330 commits, the earliest being `f7ade4a` *"Self-host: seed trck's own
backlog"* of 2026-06-05, which is the commit that created the tracker. Checked beforehand that no
path outside `issues/` ever held it — every rename in its history is `issues/…` → `issues/…`,
including the per-status-folder flattening — so `-P issues` could not have dropped an earlier
incarnation.

Nothing on `main` moved: `origin/main` is still `77d9aa4`, the working tree is clean, and a bare
`trck version` still answers with the directory. Publishing the branch is invisible until the flip.

**It starts drifting immediately.** Every tracker write from here on lands in `issues/` on `main`
and not on this branch — this very close is one of them. The flip (#8d22h6x) therefore re-runs the
split against `main` as it stands at that moment rather than pushing what is there now; what this
task established is that the operation is sound and reversible, not that the branch is current.
