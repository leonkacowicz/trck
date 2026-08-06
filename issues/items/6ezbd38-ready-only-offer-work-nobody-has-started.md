# ready: only offer work nobody has started

## Summary
`is_ready` currently offers an unblocked **`ongoing`** leaf as work to pick up. That is right
for one person working alone — "resume what you started" ought to beat starting something new —
and wrong for every other case. With several people or agents sharing a tracker, `ongoing` is
the *only* signal that a task is taken: there is no assignee field, so `start` is the claim.
Offering a claimed task to whoever asks next is precisely the failure mode `ready`/`next` exist
to prevent.

The rule that produces this is `is_actionable`, which excludes only `in-review` and `done`
(`src/trck/config.py`, `crates/trck/src/config.rs`). Its own docstring justifies excluding
`in-review` as "in flight, but its own output is pending someone else's judgement, so there is
nothing here to start" — and `ongoing` is *more* in flight than `in-review`. The two cases are
being treated differently for no reason that survives a second reader.

Narrow it: **`ready` = a `backlog` leaf with no unmet dependency.** `is_actionable` collapses
to "is backlog" for the fixed vocabulary, which is a fair sign the extra concept was carrying
the single-actor assumption all along.

What that gives up — `next` reminding a solo user of their own in-flight work — comes back
better as its own thing: `next` names what is in flight *before* the pick, so an idle picker
sees what is taken without being offered it. That lands first, so there is never a release
where the reminder is simply gone.

The second half is that `ready` then becomes coherent as a **presented** status: one definition
drives the board column, the `list` glyph, `trck ready`, `trck next` and the `ready` field in
the HTML payload, and none of them can disagree. It stays derived — nothing is stored, no verb
moves an issue "into ready".

## Acceptance criteria
- [ ] All four children are done.
- [ ] `trck ready` and `trck next` never propose an issue whose status is not `backlog`.
- [ ] `next` names in-flight work without offering it.
- [ ] Board, `list` and the JSON `ready` field all agree with `trck ready` on which issues are
      ready — no view carries a second, wider definition.

## Notes
Blast radius of the definition change: `is_actionable` in both engines, the conformance fixtures
that exercise `ready`/`next` over a started leaf, and the `ready` field emitted by
`crates/trck/src/html.rs`. It is a user-visible behaviour change, which is the right kind of
thing for the conformance suite to be the one to catch.

History: #6pvt7fy ("ready/next: honor the actionable status flag") introduced the flag when the
vocabulary was still configurable per tracker. The vocabulary is fixed in code now, so the flag
has one remaining job — parking `in-review` — and this issue decides that `ongoing` deserves
the same treatment.

Not covered here: two idle agents asking `next` at the same moment get the same deterministic
answer and can both `start` it. `start` is the claim, but the window between asking and claiming
is unguarded. That is a separate issue (`next --claim`, or accepting the race as narrow).
