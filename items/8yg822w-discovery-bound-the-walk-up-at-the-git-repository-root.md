# discovery: bound the walk up at the git repository root

## Summary

`find_tracker` walks from the working directory to `/`, and at every ancestor also scans that
directory's direct children for a `trck.json`. Nothing stops it short of the filesystem root,
so a tracker anywhere above you is one the engine will resolve and write to.

**This is no longer hypothetical. It happened in this repository, on 2026-08-16.**

```
$ cd ~/Projects/trck && trck version
tracker: /home/leon/Projects/ratchet
$ trck check
OK — 66 issues, 0 errors, 0 warning(s)
```

The walk left the repository, reached `~/Projects`, found exactly one sibling holding a
`trck.json`, and adopted it. The first symptom was `trck review r26hw48` answering `no issue
matching 'r26hw48'`, which reads as a typo rather than as *you are looking at somebody else's
tracker*.

Nothing was damaged: reads were the only verbs run before it was noticed, and the other
project's tracker is untouched. A write would have committed to it and pushed.

## The trigger is the thing that makes this urgent

That sibling had no tracker directory. It had **its own `trck-issues` branch checked out** — a
branch whose root *is* a tracker, so a `trck.json` appeared at the repository root for as long
as the checkout lasted, and vanished when it went back to `main`.

That is not an unlucky coincidence, it is the layout this project just adopted and is about to
recommend. Every repository that moves its tracker to a ref will, whenever anyone inspects that
branch, briefly present a root-level `trck.json` to any walk that passes through its parent. The
window is a checkout, not a configuration, so it opens and closes with no trace and nothing to
grep for afterwards. A colocated set of repositories under one directory — the ordinary case —
is all it takes.

It is also mutual: the same checkout makes *that* repository adopt trackers from *its* siblings.

## Why it only bit now

Before the flip (#8d22h6x) this repository had `issues/trck.json`, so the walk stopped on its
first step and never left the tree. Removing it left nothing to stop on — and a working-tree
tracker **beats** the ref in resolution order, so a stranger's directory outranked this
repository's own branch. The flip did not cause the defect; it removed what had been hiding it,
and it simultaneously created the thing that triggers it.

## The decision

**Bound the walk at the git repository root.** Recorded here rather than left open as #bxfg4vk
did, because the evidence has arrived: a tracker for *this* repository cannot be outside it, the
engine already drives git for everything else, and it is the only candidate boundary that
explains itself in one sentence. `$HOME` is arbitrary and wrong for work kept outside it; a
depth limit explains badly; warning instead of bounding leaves a wrong default people learn to
click past — and here the wrong answer looks completely ordinary.

## What it must not break

- **No git at all.** A tracker directory outside any repository is legitimate — the conformance
  suite is nothing but those, though it passes `--dir`. Where `rev-parse --show-toplevel` fails
  there is no boundary to apply, and the walk should behave as it does today rather than refuse.
- **Running from anywhere inside a repo.** The documented workflow is that `trck list` works from
  any subdirectory. Bounding at the root keeps that; bounding at the working directory would not.
- **A submodule or a secondary worktree.** `--show-toplevel` answers per checkout, which is right
  in both, but it should be stated in a test rather than assumed.
- **`--dir` and `$TRCK_DIR`.** Explicit overrides are not discovery and must keep reaching outside
  the repository, which is how a scratch tracker in `/tmp` is driven.

## Acceptance criteria
- [ ] The walk stops at the git repository root; a `trck.json` above it is never adopted.
- [ ] Reproduced first, in the shape that actually happened: two sibling repositories, the second with its tracker branch checked out, and the first resolving the second's root before the fix and not after.
- [ ] Outside a git repository the walk is unchanged, tested with a tracker that would otherwise be adopted.
- [ ] `--dir` and `$TRCK_DIR` still resolve a tracker anywhere, including outside the repository.
- [ ] When discovery finds nothing, the error says where it looked and that the search was bounded — "no tracker found here" is what sent this investigation to the wrong place first.
- [ ] Conformance states the rule, so it is behaviour rather than an implementation detail.
- [ ] `cargo test --all` passes with an unrelated tracker at `/tmp/<name>` and at the parent of the checkout.

## Notes

Supersedes the open question in #bxfg4vk, which listed the candidate boundaries and declined to
pick one. The answer is the first of them.

Worth doing in the same change: `trck version` prints the resolved tracker for a directory and
prints **nothing** for a ref, so the one command that could have answered "where is my tracker"
was silent for exactly the case that needed it. Half of this incident was the diagnostic.

Also worth a thought, separately: whether a checked-out tracker branch should be recognisable as
*a tracker branch* rather than as a tracker directory that happens to be at a repository root —
which is the ambiguity the trigger exploits.

**Until this lands, the safe form in this repository is explicit:** `trck --ref trck-issues …`.
