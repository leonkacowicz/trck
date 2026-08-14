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
- [ ] `origin/trck-issues` exists, its root is the tracker, and `trck --ref` reads it correctly.
- [ ] The split is dry-run and diffed against the working-tree tracker before pushing: same index, same bodies, same summary.
- [ ] `trck diff` resolves a pre-migration revision on the new branch (#E17).
- [ ] `main` is unchanged by this task.
- [ ] The exact commands run are recorded in the body, so the operation is auditable.

## Notes
One-shot and manual, but the verification is mechanical — the two trees must match.
