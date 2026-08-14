# docs: delete the worktree skill, rewrite both CLAUDE.md files, repoint README

## Summary
The ritual becomes the engine's business rather than the operator's, so its documentation
goes:

- `skills/trck-worktree/SKILL.md` deleted.
- Root `CLAUDE.md`: the **Tracker writes** section and the `--dir`-is-load-bearing rationale go;
  what replaces them is a paragraph on the ref, `trck sync` and the pending state.
- `issues/CLAUDE.md` moves to the tracker branch and loses the same material.
- `README.md` links `blob/trck-issues/SUMMARY.md`, since `SUMMARY.md` leaves the repo front
  page.

## Acceptance criteria
- [ ] `skills/trck-worktree/` is deleted and nothing references it.
- [ ] Neither `CLAUDE.md` mentions worktrees, `--dir`-as-a-guard, or `repo setup-git` as a precondition for filing.
- [ ] The README's tracker links resolve on the new branch.
- [ ] A reader following the new instructions can file an issue from a dirty feature branch with one command.

## Notes
Last, deliberately: docs that describe the new world while the old one is still live are worse than docs that lag by one commit.
