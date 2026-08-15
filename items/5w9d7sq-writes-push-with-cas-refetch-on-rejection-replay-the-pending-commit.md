# writes: push with CAS, refetch on rejection, replay the pending commit

## Summary
Push `<sha>:refs/heads/trck-issues` against the known remote tip. **No fetch before the
write:** a commit whose parent is not the current remote tip cannot be pushed, so either the base
was current (the validation ran against current data) or the push is rejected and it re-runs.
Fetch-on-rejection is identical in correctness to fetch-always at half the round trips.

On rejection: fetch, replay the pending operation from its trailer against the new tree, commit,
push again. Bounded retries, then report. Never force — a rejection means someone else's work
landed.

## Acceptance criteria
- [ ] The uncontended path is one network round trip.
- [ ] No write verb fetches before pushing.
- [ ] A contended push converges: two writers against one remote, both commits present afterwards, neither overwritten.
- [ ] Retries are bounded and the give-up path reports what is pending and what to run.
- [ ] `--force` never appears in a push invocation, under any path.

## Notes
Test shape follows #ey2aruc's contended-rebase test: two writers, one clone.

**Where a replayed body comes from: the pending commit's own tree, not the trailer.** #93zhqbd left
this open because neither `new`'s op nor `edit`'s records the prose, so an op replayed on its own
would produce an issue with none. It does not need to. Verified against real commits:

- `new`'s trailer carries `--id` and `--slug`, so `items/<id>-<slug>.md` is derivable from the
  trailer alone and `git show <commit>:<path>` is the body.
- `edit`'s trailer is only `edit <id>` — but the commit names the path it changed, so
  `git diff-tree --no-commit-id --name-only -r <commit>` finds it.

So the trailer records **intent** and the tree records **content**, and replay reads each from where
it lives. That split is worth keeping rather than embedding bodies in the message:

- No duplication that can disagree. A body in both places is two sources of truth for the same
  bytes, and the message copy is the one nothing validates.
- The blob is already content-addressed, so replay reuses it as-is and creates no new object.
  Re-hashing a trailer copy would reach an identical blob by a longer route, and any normalisation
  drift between the two paths becomes silent corruption.
- Trailers stay one line and small. #93zhqbd's `\n` escaping would otherwise be carrying whole
  issue bodies.

This works **because the local ref is a write-ahead log**: #jgf9ktx anchors every commit on
`refs/heads/trck-issues` before any push, which is what keeps the blob reachable and gc-proof while
the commit is pending. The anchor was there for a different reason and this depends on it.

What this does **not** answer is a content conflict — replaying an `edit` when someone else edited
the same body remotely. The bytes are available; whether they still apply is #9gxktnk's
"op that no longer applies", not a data-availability problem.
