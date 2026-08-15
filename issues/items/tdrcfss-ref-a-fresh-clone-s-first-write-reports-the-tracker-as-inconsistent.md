# ref: a fresh clone's first write reports the tracker as inconsistent

## Summary

A clone that has `origin/trck-issues` but no local `trck-issues` branch prints a consistency
failure on its first write verb. The write in fact succeeded — the commit is on the branch,
the body file is in its tree, and it was pushed:

```
$ trck new "Rehearsal: exercise the orphan branch" --priority high --body "…"

INCONSISTENCIES after this operation:
  error: #hpzc7b6 in index but no markdown file on disk
the tracker is now inconsistent — fix before committing.
origin/trck-issues:items/hpzc7b6-rehearsal-exercise-the-orphan-branch.md
```

`trck check` immediately afterwards says `OK — 29 issues, 0 errors, 0 warning(s)`, and every
later write in the same clone is clean. So the report is false, and it is the first thing a new
clone shows anyone.

## The tell

The two spellings of the printed path separate the good case from the bad one:

| Write | Path printed | Report |
|---|---|---|
| first in the clone (no local branch yet) | `origin/trck-issues:items/…` | spurious failure |
| every later one | `trck-issues:items/…` | clean |

The write resolves its source once, before the branch exists, so it is still holding the
remote-tracking ref — and the post-write check then looks for the body in a tree that predates
the commit it just made. The `Source` the check runs against needs to be the one the write
produced, not the one it started from.

## It prints twice on the replay path

A stale clone whose push is rejected reruns the operation on the fetched tip, and the message
comes out once per attempt:

```
INCONSISTENCIES after this operation:
  error: #qhtcsy2 in index but no markdown file on disk
the tracker is now inconsistent — fix before committing.

INCONSISTENCIES after this operation:
  error: #qhtcsy2 in index but no markdown file on disk
the tracker is now inconsistent — fix before committing.
origin/trck-issues:items/qhtcsy2-concurrent-filing-from-a-second-clone.md
```

Whatever the fix, the check should run once against the final state, not once per attempt.

## Reproduction

```
git clone <repo-with-a-trck-issues-branch> fresh   # no local trck-issues
cd fresh
trck new "anything" --empty                        # <- reports inconsistency
trck check                                         # <- OK
trck new "anything else" --empty                   # <- clean
```

Found rehearsing the orphan branch against a real tracker (28 issues, 39 commits of history)
in a two-clone sandbox with a throwaway remote.

## Acceptance criteria

- A first write in a clone with only a remote-tracking tracker branch reports no inconsistency.
- The consistency check runs once per invocation, including when the push was rejected and the
  operation replayed.
- Conformance covers the fresh-clone first write, since the wrong output is exactly what a user
  sees.

## Notes

Not a data bug: nothing was corrupted in any of the runs, and `check` disagreed with the message
every time. The cost is trust — a tracker that announces it is broken on first use.
