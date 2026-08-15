# diff: says 'working tree' when the tracker is a ref and there is none

## Summary

`trck diff` labels its right-hand side `working tree` unconditionally. With a ref-backed tracker
there is no working tree — the comparison is against the tip of the tracker branch — so the
header names something that does not exist:

```
$ trck diff HEAD~3
HEAD~3 → working tree
~ #qhtcsy2 status backlog → in-progress — Concurrent filing from a second clone
```

The diff itself is right. Only the label is wrong.

## What it should say

The tip of the branch the tracker resolved to, in the same spelling the write verbs already use
for their paths — `trck-issues`, or `origin/trck-issues` when that is what was resolved. That
also disambiguates the left side: `HEAD~3` is reanchored to the tracker ref, so a reader who
takes `HEAD` literally is reading the wrong history, and naming the branch on the right is the
cheapest way to say so.

## Acceptance criteria

- A ref-backed `diff` names the resolved ref on the right-hand side, not `working tree`.
- A directory-backed `diff` still says `working tree`.
- Conformance covers both spellings.

## Notes

Cosmetic, filed separately from the diagnostics gap it sits next to: with the tracker out of the
tree, no verb answers "where is my tracker" at all — `version` prints its `tracker:` line only
for a directory, and `which` is file-to-issue mapping. That is a different change; this one is
just the label.
