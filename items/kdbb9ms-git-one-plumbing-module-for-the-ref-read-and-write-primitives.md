# git: one plumbing module for the ref read and write primitives

## Summary
The ref-backed tracker needs `rev-parse --verify`, `cat-file`/`show`, `hash-object -w`,
`write-tree` against a temp index, `commit-tree`, `update-ref` with a compare-and-swap, and
`push`. Give them one home, on top of a single process-spawn wrapper, before anything grows a
second one.

`diff.rs` already does the read half: `git_run`, `git_tracker_prefix` and `git_snapshot`
(`src/diff.rs:201-241`) shell out to `git show <rev>:<path>` exactly the way a ref-backed read
wants. Move them onto the new module rather than leaving two spawn wrappers in the crate.

Decide here whether this joins `src/repo/git.rs` or sits beside it — `repo/` is the
`setup-git`/`migrate` verbs' own helpers, and the read/write path is not one of those verbs.

## Acceptance criteria
- [ ] Every git invocation in the crate goes through one wrapper that reports a missing git and a non-zero exit as a diagnostic, never a panic.
- [ ] `diff.rs` keeps its behaviour and its tests, now calling the shared module.
- [ ] The CAS forms (`update-ref <ref> <new> <old>`, `push <sha>:<ref>` against a known tip) are present and unit-tested against a temp repo.
- [ ] Nothing outside the module builds a `Command`.

## Notes
Pure groundwork: no verb changes behaviour. Sequenced first because #A4, #B5 and #C9 all assume it.
