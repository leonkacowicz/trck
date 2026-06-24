# deps: hide done work by default (fully-done chains + --omit-done)

## Summary
The `deps` graph is dominated by completed work — entire fully-done components plus done
nodes scattered through still-active chains drown out what's actually in motion. Make the
default view focus on remaining work, along **two independent axes**:

- **Fully-done chains → hidden by default.** A *chain* here is a weakly-connected component
  (`graph_components`) in which **every** node is terminal. These are never remaining work,
  so drop them. `--include-done-chains` brings them back.
- **Done nodes inside ongoing chains → shown by default.** A component that still has at
  least one open node is "in motion"; its done nodes give useful context (what already
  landed, what unblocked the live frontier), so keep them. `--omit-done` collapses the view
  to just the remaining-work frontier.

These two flags have deliberately asymmetric polarity (`--include-done-chains` is opt-in,
`--omit-done` is opt-out) because each default is independently the sensible one — both pull
the view toward "what's still in motion."

This issue is **display-only** filtering, distinct from seed selection (#hzy98pm, *where*
expansion starts) and `--depth N` (#9ax2ny2, *how far* it reaches). It supersedes the
earlier single global `--omit-completed` sketch: `--omit-done` is that flag, but the headline
change is the new default of hiding fully-done chains.

## Acceptance criteria
- [ ] By default, components whose every node is terminal are omitted from the rendered graph.
- [ ] `--include-done-chains` restores fully-done components to the output.
- [ ] By default, terminal nodes inside a component that still has ≥1 non-terminal node are
      kept and rendered.
- [ ] `--omit-done` removes terminal nodes everywhere, then **recomputes** connected
      components over the open-only node set (see "recompute, don't patch" in Notes).
- [ ] No-bridge rule honored: omitting a done node never synthesizes an edge between its
      neighbors (see Notes — the crux).
- [ ] Combining `--include-done-chains --omit-done` is defined: `--omit-done` strips done
      nodes everywhere, so a fully-done chain shown by the first flag ends up empty and thus
      effectively still hidden. Intentional, documented.
- [ ] Single-id mode (`deps <id>`): `--omit-done` is honored; `--include-done-chains` is a
      no-op (the root is already chosen, so "is the whole component done" is not a filter).
- [ ] Terminal detection uses the configured **terminal role**, not the literal word "done".
- [ ] Filters are read-only/display-only: they never mutate issues or the index.
- [ ] Tests cover: a fully-`●` component is hidden by default and restored by
      `--include-done-chains`; done nodes in a mixed component render by default and vanish
      under `--omit-done`; `--omit-done` re-splits a component into the right open-only
      components (an open node whose blockers are all done becomes its own root/singleton);
      omitting a mid-chain done node does NOT invent an edge between its neighbors;
      `--include-done-chains --omit-done` yields the documented empty/hidden result.

## Notes
Design context (from discussion):

- **No-bridge rule for omitting done nodes (the key decision).** For chain `C → B → A`
  (C depends on B, B depends on A) with **B completed**: removing B must simply delete B
  and its edges — do **not** synthesize `C → A`. Rationale: the edge `C → B` only meant
  "B must precede C"; B's own dependency on A only ever mattered as a way to *enable* B.
  Once B is done, B is satisfied regardless of whether A ever was, so A no longer gates
  anything downstream of B. Synthesizing `C → A` would invent a dependency the data never
  asserted and that completion has made moot. **Out-of-order completion is the same case:**
  if B is marked done while A is still open, we still do **not** promote A to a dependency
  of C — a transitive-only link is never materialized; only edges the user set explicitly
  exist. If a real C→A constraint exists, state it as a direct edge — the tool must not
  fabricate one. Bonus: this also keeps a future `--actionable` honest (with B done, C's
  only gate is satisfied, so C is actionable; a fabricated `C → A` would wrongly re-block C).
- **Recompute, don't patch.** `--omit-done` is not "delete done nodes from existing
  components and fix up dangling edges." It is: (1) drop terminal nodes from the node set,
  (2) **recompute** `graph_components` over the open-only subgraph. A done dependency is a
  *satisfied* dependency, not a pending blocker, so dropping its edge loses no information;
  an open node left with all-done blockers naturally falls out as its own root/singleton —
  which is correct, it's *ready*. No orphan handling, no edge stubs. This reuses the existing
  `graph_components` machinery — just feed it the open-id set.
- **Completion-pruning ≠ cosmetic-pruning.** The no-bridge rule is correct *because the
  pruned node is terminal*. Hiding an *incomplete* node for declutter (e.g. "hide all
  sub-tasks") is the opposite case: there, dropping edges would lie (C would look unblocked
  when it isn't), so a generic property-prune would need bridging. Keep this done-filtering
  as its own honest thing rather than folding it into a generic "omit by property" flag —
  the two have genuinely different correct semantics.
- **Mental model:** `--omit-done` → "graph of remaining work"; default → "remaining work plus
  the done context that led to it"; fully-done chains → never remaining work, hidden unless
  `--include-done-chains`.
- **Follow-ups (own issue if/when wanted):** `--actionable` / `--blocked` (open issues whose
  deps are all terminal vs. still blocked), and `--omit-isolated` (drop singleton nodes).
- Touchpoints: `graph_components`, `_graph_component_rows`, `_print_deps_graph`, `cmd_deps`;
  terminal-status detection via the configured terminal role.
