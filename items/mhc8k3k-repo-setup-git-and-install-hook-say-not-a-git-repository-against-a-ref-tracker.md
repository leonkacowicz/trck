# repo setup-git and install-hook answer 'not a git repository' against a ref tracker

## Summary
Both git-touching `repo` verbs route every git call through `repo::git::git`, which is
`crate::git::stdout(ctx.dir()?, args)` (`src/repo/git.rs:10`). `ctx.dir()` has no answer for a
`Source::Ref`, so the call fails before git is ever spawned — and `require_repo`
(`src/repo/git.rs:18`) maps *any* failure to `not a git repository`, which is the one thing that
is definitely not wrong: the repo is right there, it is the tracker that has no directory.

Against a ref-backed tracker (`issues/` deleted, tracker read from `trck-issues`):

```
trck repo setup-git      -> error: not a git repository        (exit 1)
trck repo install-hook   -> error: not a git repository        (exit 1)
trck repo migrate-layout -> error: the tracker is git ref 'trck-issues', which has no files on disk
trck repo normalize      -> error: the tracker is git ref 'trck-issues', which has no files on disk
```

The last two are honest — `ctx.dir()`'s own error reaches the user. The first two are the same
condition wearing a wrong sentence.

**`setup-git` is not merely mis-worded, it is needed there.** Its refspec half exists *for* this
shape: `discovery::refspec::hidden` (`src/discovery/refspec.rs:59`) tells a clone whose
`remote.origin.fetch` does not cover the branch to `run `trck repo setup-git``, and that clone's
tracker is a ref by definition. So the engine's own diagnostic names a verb that cannot run —
and worse, that clone cannot resolve a tracker at all, so the verb would fail at discovery even
if `ctx.dir()` were not in its way. Widening a refspec and registering merge drivers are both
per-clone `.git/config` writes that want nothing from a tracker directory.

`install-hook` is the opposite case: a pre-commit hook is genuinely meaningless for a ref-backed
tracker — no commit in the working tree can make it inconsistent, which is why this repo's own
guard stopped running `trck check` (#tkhcgv6). It should refuse *saying that*, not by claiming
the repository is not one.

## Acceptance criteria
- [ ] `repo setup-git` works against a ref-backed tracker: registers the merge drivers and
      widens `remote.origin.fetch`, without needing a tracker directory.
- [ ] Its `.gitattributes` half is decided explicitly — written to the branch root through the
      write path, or skipped with a reason — rather than being what makes the verb fail.
- [ ] A clone whose refspec hides the branch can run the verb the `hidden` diagnostic names,
      from a state where the tracker does not resolve.
- [ ] `repo install-hook` refuses a ref-backed tracker with a message that says why a hook has
      nothing to guard, not `not a git repository`.
- [ ] `require_repo` no longer swallows a non-git failure into that sentence.
- [ ] Conformance fixtures for both verbs against a ref tracker — this is output a user sees.

## Notes
Found while documenting the migration in the README (the `git subtree split -P issues -b
trck-issues` recipe), where the single-branch-clone bullet had to point at a refspec line rather
than at the verb, because the verb does not run.

Related: #yqb2afy added the refspec widening; #tkhcgv6 is why the hook is moot on a ref tracker.
