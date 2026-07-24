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
