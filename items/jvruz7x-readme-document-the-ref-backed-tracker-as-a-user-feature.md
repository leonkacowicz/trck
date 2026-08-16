# README: document the ref-backed tracker as a user feature

## Summary

v0.30.0 shipped a second place a tracker can live, and the README does not mention it. A
reader learns that a tracker is `issues/` with a `trck.json` in it, and nothing tells them
the other half exists — so the feature is discoverable only by reading this repository's
own branch layout and guessing.

What is missing, roughly in the order a user meets it:

- **`--ref` and `$TRCK_REF`**, and the conventional `trck-issues` branch when neither is given.
- **The resolution order**, which is the part that surprises: `--dir` → `$TRCK_DIR` → `--ref`
  → `$TRCK_REF` → a tracker directory in the working tree → the conventional ref. A directory
  wins over a ref, deliberately — it is what lets a repository move over in pieces.
- **Local versus `origin/`**: with a local branch it is used (fast-forwarded first if it is
  behind); with none, `origin/trck-issues` is read; diverged is reported and named.
- **Writes**: one commit per verb, pushed, replayed onto whoever landed first if rejected,
  never forced. `Trck-Op:` is what makes the replay possible.
- **`trck sync`** and the `(N unpushed changes)` report — a write that could not reach the
  remote succeeded, and the difference has to be visible.
- **`trck edit`**, `--body`/`--body-file`/`--empty`, and `$VISUAL`/`$EDITOR`: prose without a
  file to open.
- **What a ref-backed tracker cannot do**: `path`, `which` and `list --paths` refuse, because
  there are no files. Worth stating plainly rather than leaving to be discovered.
- **How to move an existing tracker onto a branch** — `git subtree split`, verify the trees
  match, push, then remove the directory. This repository did exactly that (#usc2cxg,
  #8d22h6x) and the commands are in those bodies.

## Acceptance criteria
- [ ] The README has a section covering the ref-backed tracker, at the depth the rest of the README uses.
- [ ] The resolution order is stated explicitly, including that a directory beats a ref.
- [ ] The verbs that refuse against a ref are named, with the reason.
- [ ] The migration is written out as a procedure someone can follow on their own repo.

## Notes

Not part of the flip (#wzg85n6 covered repointing the links and deleting the worktree
ritual). This is the user-facing half, and it is the one an outside reader needs — the
maintainer-facing half is already in `CLAUDE.md`.

Worth considering alongside a `trck repo split-branch` verb: the migration section is a
procedure precisely because there is no verb for it, and the verification step — that the
split tree and the working-tree tracker are the same object — is the part a user will skip.
