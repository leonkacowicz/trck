# check: warn on authored dependency edges made redundant by another path

## Summary

The counterweight to #atk4umk. Transitive reduction hides redundant edges from the graph;
on its own that quietly papers over sloppy data forever, and the index accumulates cruft
that only bites when someone later removes the covering edge and a hidden constraint
silently reappears.

So `trck check` should *say so*: for each authored edge implied by another authored path,
emit a warning naming the covering path, so it can be cleaned deliberately.

```
warning: #A's dependency on #C is implied via #B — consider `trck dep A --remove C`
```

Warning, never error — a redundant edge is untidy, not invalid, and `check` gates commits.

## Acceptance criteria
- [ ] `trck check` warns once per redundant **authored** edge, naming a covering path.
- [ ] Warning only; exit status unchanged, so `check` still passes.
- [ ] Inferred edges (#zhhxgcw, #zbkkc2a) never trigger a warning — nobody authored them
      and there is nothing to remove. See the note below; this is the subtle part.
- [ ] The message includes the exact `trck dep … --remove …` invocation to fix it.
- [ ] Tests: redundant edge warns, non-redundant does not, inferred coverage does not.

## Notes

- **Which graph to compute redundancy over is the whole design question.** Over the
  authored-only graph, the warning means "you wrote an edge you did not need" — actionable
  and safe. Over the combined authored + inferred graph, an edge authored on a parent is
  covered by paths through its own children (the #zbkkc2a fanout), so `check` would advise
  removing precisely the parent-altitude edges the docs tell you to prefer. **Use the
  authored-only graph.**
- That divergence is deliberate: the *renderer* reduces the combined graph, the *linter*
  reduces the authored one. They answer different questions — "what should I draw" vs
  "what did you write that you did not need" — and conflating them inverts the advice.
- `validate` (`trck:960`ff) is the home for this; it already returns a `(errors, warnings)`
  pair, so this is additive.
- Depends on #atk4umk only for the shared reduction primitive — factor it so both callers
  use one implementation parameterised by which edge set to reduce.

## Resolution: wontfix

Built, then reverted. The premise above is wrong.

The stated motivation — that hiding a redundant edge lets the index accrue cruft "that only
bites when someone removes the covering edge and the hidden constraint silently reappears" —
does not describe a failure. If `A -> C` is hidden while `B -> C` covers it, and `B -> C` is
later removed, `A -> C` simply becomes visible again. That is correct behaviour. The reasoning
dressed up a feature as a hazard and then proposed a linter to prevent it.

Worse, the advice destroys information. `A -> C` and `B -> C` are independent assertions: one
says *A* needs C, the other says *B* does. Their redundancy is contingent on `B -> C`
continuing to exist. Remove `A -> C` as advised, later rescope `B` so it no longer needs `C`,
and A's genuine constraint is gone with nothing recording that it was ever stated. The warning
trades a robust explicit fact for a fragile inferred one — the opposite of the usual
convention, where you declare what you directly depend on precisely *because* transitive
availability is someone else's decision to change.

The display-side reduction (#atk4umk) does not share the problem, which is why the two looked
alike and are not. It declutters a view recomputed from scratch every time and touches nothing
durable; this would have mutated stored data to match a property that is not durable.

A signal misread while implementing: when the warning fired on this tracker's own data, the
instinct was to *suppress* it (skip terminal issues) rather than act on it. That the advice was
not worth taking was the actual finding.

Reverted in full; `edge_reach` / `implied_edges` / `transitive_reduction` stay, since the
renderer needs them. If this redundancy is ever worth surfacing, it is a display concern, and
the display already handles it.
