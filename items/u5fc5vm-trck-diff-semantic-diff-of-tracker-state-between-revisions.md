# trck diff: semantic diff of tracker state between revisions

## Summary
`index.jsonl` is generated, so its raw `git diff` is unreadable JSON churn — a status move and a
title rewrite look the same. `changelog` only covers timestamped events (created / started /
closed) and is blind to priority, parent, label, dep, kind, and points edits.

`trck diff` closes that gap: load the tracker at two points, join rows by id, and render *what
actually changed* in the tracker's own vocabulary. Default comparison is `HEAD` vs the working tree;
`trck diff main` answers "what did this branch do to the backlog?".

**The engine is VCS-agnostic and git is a convenience layer on top.** Everything below the source
seam (#q9cq65c) consumes *snapshots* and has no idea where they came from; the git provider
(#wtmfdhr) turns revision specs into snapshots and is the only place `subprocess` appears. So
`trck diff --from old-index.jsonl` works with git absent, under jj or hg, or on an unpacked backup —
and every renderer is testable from fixture files with no git repository involved.

Output is layered on git's own gradient — `--stat` (counts), default (epic-rollup), `--flat`
(ledger), `-v` (field blocks) — so the flags cost no new vocabulary.

## Acceptance criteria
- [ ] All children done.
- [ ] `trck diff` works from any revision spec git accepts, and against a dirty working tree.
- [ ] `trck diff` works with git absent from `PATH` and outside any repository, via `--from`.
- [ ] Rendering degrades cleanly with `NO_COLOR` / non-tty (reuse `render.paint`).

## Notes
- `subprocess` + git is already used in `cmd_maint.py` (`setup-git`, `install-hook`), so the git
  provider adds no new dependency and does not touch the stdlib-only rule. But note those are both
  explicitly git-flavoured verbs that already `die("not a git repository")` — `diff` would be the
  first *reading* verb to need history, which is why the dependency is isolated behind a seam
  instead of assumed.
- `merge.py` already models row-wise comparison keyed by id for the 3-way merge driver — `diff` is
  the read side of the same coin. Check whether `_tuple_of` / `_by_id` can be shared rather than
  re-derived.
- `--json` output belongs to the `--json` epic (#r9zefup) and its shared stdout seam (#v8tmkrt),
  not here. Add the edge once this epic's model exists.
- Deliberately out of scope for now: a "grouped by event kind" layout (Created / Started / Closed /
  Reprioritized sections). It reads well as release notes but double-lists issues that changed in
  two ways; revisit after the four layers below are in.
