# deps: graph display filters (--omit-completed, --depth N)

## Summary
Add display-side pruning to the `deps` graph — distinct from seed selection (#055).
Whereas seeds choose *where expansion starts*, these filters prune *what gets drawn*.
Two to ship first, covering the two real complaints (done-noise, oversized graphs):

- **`--omit-completed`** — remove terminal-status nodes from the rendered graph.
- **`--depth N`** — cut expansion N hops from the seed(s); a property of the traversal,
  so it composes cleanly with the #055 seed union.

Strong follow-ups (own issue if/when wanted, not required here): `--actionable` /
`--blocked` (the dependency-graph-specific filter: open issues whose deps are all
terminal vs. those still blocked), and `--omit-isolated` (drop singleton nodes).

## Acceptance criteria
- [ ] `--omit-completed` removes terminal-status nodes; it does **not** transitively
      bridge across a removed node (see the no-bridge rule in Notes — this is the crux).
- [ ] `--depth N` limits rendering to nodes within N hops of a seed; composes with the
      multi-seed union from #055.
- [ ] Filters are read-only/display-only: they never mutate issues or the index.
- [ ] Naming keeps these clearly separate from the #055 seed predicate.
- [ ] Tests cover: `--omit-completed` drops a mid-chain done node WITHOUT inventing an
      edge between its neighbors; `--depth` truncates at the right hop count; the two
      compose with a multi-seed graph.

## Notes
Design context (from discussion):

- **No-bridge rule for `--omit-completed` (the key decision).** For chain `C → B → A`
  (C depends on B, B depends on A) with **B completed**: removing B must simply delete B
  and its edges — do **not** synthesize `C → A`. Rationale: the edge `C → B` only meant
  "B must precede C"; B's own dependency on A only ever mattered as a way to *enable* B.
  Once B is done, B is satisfied regardless of whether A ever was, so A no longer gates
  anything downstream of B. Synthesizing `C → A` would invent a dependency the data never
  asserted and that completion has made moot. If a real C→A constraint exists, it should
  be stated as a direct edge — the tool must not fabricate one. Bonus: this is also what
  keeps a future `--actionable` honest (with B done, C's only gate is satisfied, so C is
  actionable; a fabricated `C → A` would wrongly re-block C on a stale node).
- **Completion-pruning ≠ cosmetic-pruning.** The no-bridge rule is correct *because the
  pruned node is terminal*. Hiding an *incomplete* node for declutter (e.g. "hide all
  sub-tasks") is the opposite case: there, dropping edges would lie (C would look
  unblocked when it isn't), so a generic property-prune would need bridging. That is a
  reason to keep `--omit-completed` as its own honest thing rather than folding it into a
  generic "omit by property" flag — the two have genuinely different correct semantics.
- **Open question:** what to do with a node left isolated after its completed neighbors
  are removed — show it, or drop it too? (Relates to a possible `--omit-isolated`.)
- Touchpoints: the Graph render/scoping path (#046/#047); terminal-status detection must
  use the configured terminal role, not the literal word "done".
