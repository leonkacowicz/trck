# discovery: should the walk up be bounded, or can it adopt a stranger's tracker

## Summary
`find_tracker` walks from the working directory to `/`, and at **every** ancestor scans that
directory's direct children for a `trck.json`. There is no stopping point short of the filesystem
root. So a tracker anywhere above you, in any directory you merely happen to be beneath, is one the
engine will resolve and act on.

With `~/projects/a/issues/trck.json` present, running `trck list` from `~/projects/b` — a directory
with no tracker of its own — reports project A's issues. A `trck done` there closes one of project
A's. Nothing warns, because from the engine's point of view it found exactly what it was looking
for.

This is the same mechanism that made #jvk5637 a test problem, and #jvk5637 fixed only the half that
was ours: no fixture of this repository puts a `trck.json` where it can be adopted. The other half —
a tracker somebody *else* left above you — cannot be fixed by any placement of our own files, which
is why it is here rather than there.

**How far it reaches.** It is not only the three "nothing to find" assertions. A tracker planted at
`/tmp/<name>` breaks the whole ref-backed integration suite, because those fixtures depend on
discovery walking up, finding nothing, and falling through to the conventional ref. `tests/ref_diff.rs`
fails with `unknown revision`. That is a fair proxy for the user-facing shape: anything relying on
"there is no tracker here" silently becomes "there is, and it is someone else's".

## Acceptance criteria
- [ ] A decision is recorded, with its reasoning, even if the decision is to keep the walk unbounded.
- [ ] If bounded: `trck list` from a directory with no tracker of its own does not resolve one that only shares a distant ancestor.
- [ ] If bounded: the behaviour every documented workflow relies on still holds — running from anywhere inside a repo finds that repo's `issues/`.
- [ ] Whatever is decided, `cargo test --all` passes with an unrelated tracker sitting at `/tmp/<name>` — the acceptance criterion #jvk5637 could not meet.
- [ ] The conformance suite says what the rule is, so it is behaviour rather than an implementation detail.

## Notes
Split out of #jvk5637, whose AC 4 deliberately put this out of scope: the sibling scan is behaviour
users rely on, and changing it is not something a test-isolation fix should do quietly.

Candidate boundaries, none obviously right:

- **The git repository root.** Matches how the tracker is used and how `setup-git` already thinks.
  Breaks a tracker kept outside a repository, which nothing forbids today.
- **`$HOME`.** Stops the `/tmp` and cross-project cases without knowing about git. Arbitrary, and
  wrong for a machine that keeps work outside `$HOME`.
- **A depth limit.** Simple, and explains badly — "why did it stop at three?"
- **Nothing, and warn instead.** Keep resolution as it is but say which tracker was chosen and why
  when it came from an ancestor rather than the working directory. Cheapest, and turns a silent
  wrong answer into a visible one, which may be the whole of the problem.

The last option is worth weighing seriously: the failure here is not that the tracker is far away,
it is that nothing says so.
