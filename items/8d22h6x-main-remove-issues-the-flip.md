# main: remove issues/ — the flip

## Summary
```
git rm -r issues/    # on main, as its own commit
```

This is the atomic point of the epic. Until now the working-tree tracker has won every
resolution and nothing has changed; after this commit, discovery falls through to the ref and
every verb is exercising the new path for real.

Everything it depends on is in by construction: reads from the ref (#B6-#B8), writes through
plumbing (#C9-#C13), body editing without a checkout (#D16), `diff` on the tracker branch
(#E17), and CI and hooks that no longer look in the tree (#E19, #E20).

## Acceptance criteria
- [ ] `issues/` is gone from `main` in a commit that touches nothing else.
- [ ] Every read and write verb works from a fresh clone of `main` with no arguments.
- [ ] A fresh clone with no local `trck-issues` reads `origin/trck-issues` (the absent row of #B7's table).
- [ ] `trck check` passes against the ref.

## Notes
**Risk to state plainly:** every machine running an older `trck` stops finding the tracker at this commit. It errors rather than reporting an empty tracker, which is the acceptable failure — but it is a hard cutover for every clone and every agent harness, and it wants an announcement rather than a discovery.
