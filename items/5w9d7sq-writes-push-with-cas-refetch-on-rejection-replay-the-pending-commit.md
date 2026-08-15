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
- [x] The uncontended path is one network round trip.
- [x] No write verb fetches before pushing.
- [x] A contended push converges: two writers against one remote, both commits present afterwards, neither overwritten.
- [x] Retries are bounded and the give-up path reports what is pending and what to run.
- [x] `--force` never appears in a push invocation, under any path.

## Notes
Test shape follows #ey2aruc's contended-rebase test: two writers, one clone.

Landed in PR #43. `src/verbs/backend/sync.rs` is the push and the rejection loop;
`src/verbs/replay/` turns an `Op` back into the verb call that produced it.

The AC that carries the design is the contended one, and its test proves the part that matters:
two writers close **one child each** of a shared epic, and the epic ends up `done`. That only
happens if the rebuilt `done` derived its rollup against the other writer's closed child as well
as its own — neither writer's own version ever said `done`, so a textual merge could not have
produced it. Derivation cannot be merged; it has to be redone, which is why replay re-runs the
verb rather than merging `index.jsonl`.

A failed push **fails the verb**, with a message leading on git's own reason, saying the work is
committed locally and not lost, and naming the command that sends it. Every failure path is
wrapped, not just retry exhaustion — an unreachable remote fails at the fetch in git's words,
which say nothing about the issue just filed. If a pending write should be a *report* rather than
an error, `sync` is where #dak2sjq softens it.

Re-entrancy is guarded with an `AtomicBool`: rebuilding re-runs the verb and lands back in
`commit`, and a nested push loop would have every rejection start another. The outer loop is the
only thing that pushes. Pragmatic rather than structural — worth knowing if #9gxktnk reshapes the
loop.

Two `ref_standing` tests needed new setup, not new intent: they build a local branch ahead of the
remote and called it "filed offline", which was free while nothing pushed. It now takes a
genuinely unreachable remote.

**Found and left as designed:** an issue this clone has never fetched cannot be referenced —
`--parent <id>` resolves against local rows and refuses before a push could teach it otherwise.
That is inherent to "no fetch before the write".

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
