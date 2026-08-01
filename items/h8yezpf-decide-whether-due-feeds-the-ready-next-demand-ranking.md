# decide whether due feeds the ready/next demand ranking

## Summary
`ready`/`next` rank by the demand cone (#9bktptp): an issue's own priority, lifted by the priority
of everything it transitively unblocks. It is a closed, well-understood model — two inputs
(dependencies, priority), no clock. A deadline would be a third axis, and a time-varying one.

Decide whether `due` participates, and record why either way. This is the design question the rest
of the due-date epic is deliberately kept independent of.

**The case for:** a deadline is precisely the information "what should I work on next" wants, and
it propagates naturally along the existing edges — a blocker of a due-Friday task inherits the
deadline exactly as it inherits urgency today.

**The case against:** priority already expresses "this should come first", and the demand cone
already propagates it. Adding a deadline gives two overlapping soft signals with no rule for
which wins, and makes ranking depend on *when you ran the command* — the same output-varies-with-
now problem the rest of this epic is careful to keep out of generated artifacts. Nothing enforces
a due date, so a stale one silently distorts the ordering of everything behind it.

**Middle options:** let overdue-ness act as a priority *lift* (bounded, one step) rather than a
ranking term; or keep ranking untouched and expose `ready --overdue` / `--sort due` so the user
opts in explicitly.

## Acceptance criteria
- [ ] The decision is made and written down here, with the reasoning.
- [ ] If yes: the interaction with the demand cone is specified precisely (how a due date
      propagates, how it composes with priority, what the marker on a lifted row says) and a
      follow-up issue is filed to implement it.
- [ ] If no: the alternative (explicit opt-in via filter/sort) is confirmed as sufficient and the
      `ready`/`next` docs say why deadlines don't rank.

## Notes
- Ranking lives in the demand-cone work: #5yjce3w (the vector), #yrre4zn (ranking by it),
  #aujt85q (the `↑<priority>(#culprit)` marker).
- Needs [[x6argpr]] — no point specifying the interaction before the field exists.
- Initial lean: the explicit opt-in. Determinism of `next` is worth more than automatic deadline
  pressure.
