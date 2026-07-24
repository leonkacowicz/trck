# deps: hoist uniform inherited fanouts back to the parent; --fanout to opt out

## Summary

Post-pass over the reduced graph: where **all** children of `P` carry an inherited edge to
the same target `X`, collapse the fanout back to a single `P -> X`. This is the inverse of
the lifting rule — "if every child needs X, that is a property of the parent" — and it
restores dependencies to the altitude they were authored at.

It exists because #zbkkc2a + #atk4umk together demote every parent-authored edge into `n`
child edges (see that issue for the worked example). The hoist is the fix, and it lands on
the right answer in both directions: a per-child inherited edge survives only when it is
**not** uniform across siblings, which is exactly when the per-child detail is informative.

Two views, one code path — the hoist is a post-pass, so opting out is just skipping it:

- **default** — hoisted. Edges sit where they were authored; `P -> X` reads as "this epic,
  as a whole, waits on X".
- **`--fanout`** — not hoisted. `P -> X` is gone, replaced by `C1 -> X … Cn -> X`. Nothing
  is lost: `P` still reaches `X` transitively through its children, and the per-child edges
  are the ground truth about which specific work is blocked.

## Acceptance criteria
- [ ] A uniform inherited fanout renders as one parent-altitude edge by default.
- [ ] A non-uniform one keeps its per-child edges.
- [ ] `--fanout` skips the hoist and shows every inherited edge per child.
- [ ] The hoist runs after reduction, and re-running it is idempotent.
- [ ] Tests: uniform fanout, partial fanout, single-child parent, nested parents.

## Notes

- Naming: **not** `--verbose`. Every existing `deps` flag is semantic (`--omit-done`,
  `--include-done-chains`, `--full`); `--verbose` reads like a log level and says nothing
  about which view you get. `--fanout` (or `--per-child`) names the output.
- Reduction and hoist are separate axes; the two presets above pin three of the four
  corners. The fourth — reduction off entirely, every inferred edge drawn — is useful when
  debugging the renderer itself, but is a footgun on any real graph. Build it as an internal
  toggle the tests can reach; do not expose it as a flag without a demonstrated need.
- Uniformity is over the parent's **direct** children. Whether a grandparent's fanout should
  hoist through two levels at once is unresolved — the recursive case probably falls out of
  applying the pass bottom-up, but confirm with a nested test rather than assuming.
