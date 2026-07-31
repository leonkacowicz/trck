# diff: git convenience layer — revision specs, default HEAD vs working tree

## Summary
The ergonomic layer over the source seam (#q9cq65c): make the common case one word.

```
trck diff                 # HEAD vs working tree — "what have I not committed?"
trck diff main            # main vs working tree — "what did this branch do to the backlog?"
trck diff v0.22..v0.23    # between two tags — release notes with metadata edits included
trck diff HEAD~5          # the last five commits' worth of tracker movement
```

A bare positional argument is interpreted as a revision spec and resolved into a snapshot via
`git show <rev>:<tracker-relative-path>`. Two-dot form supplies both sides. With no argument, the
old side is `HEAD` and the new side is the working tree.

This is a **provider**, not a special case: it produces the same snapshot object as `--from`, so
nothing downstream changes behaviour depending on whether git was involved.

## Acceptance criteria
- [ ] `trck diff` with no arguments diffs `HEAD` against the working tree.
- [ ] A positional revision spec resolves via `git show`; `<a>..<b>` sets both sides.
- [ ] The tracker path handed to `git show` is repo-relative, derived the way `install-hook` already
      derives it (`git rev-parse --show-toplevel` + `relative_to`), so it works from any subdirectory
      and for a tracker dir that is itself the repo root.
- [ ] The tracker dir absent at that revision is **not** an error — every issue reads as added.
- [ ] Bodies are available from a git snapshot (`git show <rev>:<tracker>/items/<file>`), so #6xcseef
      can detect body edits on the old side.
- [ ] Clear, distinct errors for: git not on `PATH`, not a git repository, and an unresolvable
      revision. None of them a traceback.
- [ ] Revision specs are rejected when git is unavailable with a message pointing at `--from`.

## Notes
- Depends on #q9cq65c; this issue must add no new concept below the seam.
- `subprocess` is already imported in `cmd_maint.py` and used for git, so no new dependency and no
  change to the stdlib-only rule.
- Only fetch what is needed: reading every body at a revision costs one `git show` per issue.
  Fetching bodies lazily (on first `body(id)` call) keeps `trck diff` cheap when `--body` is off,
  which is the default.
- `git cat-file --batch` could fetch many blobs in one process if the per-issue cost ever bites.
  Not worth it up front — measure first.
