# mv/done: guard closing an issue with unfinished dependencies (out-of-order completion)

## Summary
When an issue B is moved to a terminal status while B still has a **non-terminal
dependency** A, that is an out-of-topological-order completion: B is declared done even
though something it was declared to depend on isn't. Surface this at the moment it
happens (the `mv`/`done` transition) rather than silently.

Caught at mutation time this is more valuable than any view-side treatment, because it
flags the anomaly as it is created. Sibling of #018 (which guards closing a *parent* with
open *descendants*); this one guards closing an issue with open *dependencies*.

The user-facing strictness — warn-only vs. hard-block-with-`--force` — is **deliberately
left open**; decide when this issue is planned (see Notes).

## Acceptance criteria
- [ ] Moving an issue to a terminal status while it has ≥1 dependency in a **non-terminal**
      status is handled deliberately (strictness TBD: warn vs. block + `--force`).
- [ ] The condition keys on "dependency not in a terminal status" — vocabulary-agnostic,
      using the configured terminal role. A dep that is terminal-but-not-done (e.g.
      `wontfix`/`duplicate`) is satisfied-by-abandonment and triggers **nothing**.
- [ ] The message names the specific unfinished dependency/dependencies (not a vague
      warning) so the user can actually verify.
- [ ] If the transition newly unblocks one or more successors, the message additionally
      names them — but only asserts "now actionable" after confirming the successor has
      **no other** non-terminal dependency ("last blocker" check); otherwise it says
      "one step closer" / stays silent on actionability.
- [ ] Tests cover: closing with an open dep triggers; a terminal-but-not-done dep does
      not; the named-deps message is correct; the "last blocker" actionability claim is
      gated on the successor's full dep set; (if block chosen) `--force` overrides.

## Notes
Design context (from discussion):

- **Two-tier message.** (1) *Unconditional:* "closing B, but it depends on A which isn't
  done." (2) *Conditional:* if closing B newly unblocks successor C, append that C is now
  actionable — only when C has no other pending dep.
- **"Last blocker" needs C's whole dep set.** C becomes actionable iff *all* of C's deps
  are terminal, not just B. Verify before asserting actionability, else downgrade the
  wording.
- **"Non-terminal," not "not done."** Reuse the terminal-role config (covers
  `done`/`wontfix`/`duplicate`) so abandoned deps don't false-positive.
- **Open question — warn vs. block + `--force`.** Leaning toward a non-blocking warning
  (dependency data is often aspirational/stale; A may genuinely be unnecessary, and "if
  they really need A before C they'd state it directly"). A hard block with `--force`
  escape is the stricter alternative. Decide before implementing — mirror whatever #018
  lands on for consistency between the two guards.
- **Possible `check` lint (safety net).** "A terminal node with a non-terminal
  dependency" is also a *standing* graph property, not just a transition event, so
  `trck check` could report it as a soft lint — catching cases created before this guard
  existed or via hand-edits/imports. The live `done`-time guard is primary; the lint is
  the net. Decide scope when planning.
- Touchpoints: `cmd_mv` / `move_issue` / the `done` alias path; the dependency walk
  (reuse with a cycle guard); argparse for `mv`/`done` if `--force` is chosen.
