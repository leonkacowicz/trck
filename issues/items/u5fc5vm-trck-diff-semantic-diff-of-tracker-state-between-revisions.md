# trck diff: semantic diff of tracker state between revisions

## Summary
`index.jsonl` is generated, so its raw `git diff` is unreadable JSON churn — a status move and a
title rewrite look the same. `changelog` only covers timestamped events (created / started /
closed) and is blind to priority, parent, label, dep, kind, and points edits.

`trck diff [<rev>[..<rev>]]` closes that gap: load the tracker at one or two revisions, join rows
by id, and render *what actually changed* in the tracker's own vocabulary. Default comparison is
working tree vs `HEAD`; `trck diff main` answers "what did this branch do to the backlog?".

Output is layered on git's own gradient — `--stat` (counts), default (epic-rollup), `--flat`
(ledger), `-v` (field blocks) — so the flags cost no new vocabulary.

## Acceptance criteria
- [ ] All children done.
- [ ] `trck diff` works from any revision spec git accepts, and against a dirty working tree.
- [ ] Rendering degrades cleanly with `NO_COLOR` / non-tty (reuse `render.paint`).

## Notes
- Reading the old side is `git show <rev>:<tracker>/index.jsonl`; `subprocess` + git is already
  used in `cmd_maint.py` (`setup-git`, `install-hook`), so no new dependency.
- `merge.py` already models row-wise comparison keyed by id for the 3-way merge driver — `diff` is
  the read side of the same coin. Check whether `_tuple_of` / `_by_id` can be shared rather than
  re-derived.
- `--json` output belongs to the `--json` epic (#r9zefup) and its shared stdout seam (#v8tmkrt),
  not here. Add the edge once this epic's model exists.
- Deliberately out of scope for now: a "grouped by event kind" layout (Created / Started / Closed /
  Reprioritized sections). It reads well as release notes but double-lists issues that changed in
  two ways; revisit after the four layers below are in.
