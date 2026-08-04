# rust: ready and next, with the demand annotations

## Summary
The "what should I pick up" surface, ranked by demand rather than declared priority alone.

## Acceptance criteria
- [x] Actionable leaves only: not terminal, not blocked directly or by inheritance, not parked
      in a non-actionable state.
- [x] Demand ranking, then the existing `-points`, `id` tie-breaks.
- [x] The `↑<priority>(#id)` marker, emitted exactly when a row is lifted above its own
      priority and never otherwise.
- [x] Subtree scoping that narrows the result without narrowing the graph readiness is computed
      over — narrowing the graph makes blocked work look actionable.
- [x] `next` as the single pick.

## Landed
`77cfba0`. Conformance 10/12 -> 11/12; the remaining failure is `deps` (`bdmgj7r`).

The ranking was already in place — `ranked_ready` landed with the graph (`ehqv6sk`) and
was verified against Python's over this repo's real 195-issue tracker. What is new here is
the verb, the subtree scoping, and the note.

**The differential sweep passed while testing nothing that mattered.** 14 invocations over
both real trackers, byte-identical — but nothing in this repo is currently lifted above its
own priority, so the demand annotation, the part most likely to be subtly wrong, was never
exercised. Built a scenario in both engines that does exercise it (a medium blocker under
an urgent dependent, and an epic ranking its own lowest-priority leaf); both `↑urgent(#…)`
notes match. The new fixture pins it so nobody has to remember to construct it again.

Worth generalising: a green sweep over real data proves less than it looks when the real
data happens not to contain the interesting case.

**The row annotation became an enum.** `list` explains what a row is waiting on, `ready`
explains why it ranks where it does, and they share one slot — so the choice is three-way,
and the bool it replaced was about to become a lie.
