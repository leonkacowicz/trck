# rust: ready and next, with the demand annotations

## Summary
The "what should I pick up" surface, ranked by demand rather than declared priority alone.

## Acceptance criteria
- [ ] Actionable leaves only: not terminal, not blocked directly or by inheritance, not parked
      in a non-actionable state.
- [ ] Demand ranking, then the existing `-points`, `id` tie-breaks.
- [ ] The `↑<priority>(#id)` marker, emitted exactly when a row is lifted above its own
      priority and never otherwise.
- [ ] Subtree scoping that narrows the result without narrowing the graph readiness is computed
      over — narrowing the graph makes blocked work look actionable.
- [ ] `next` as the single pick.
