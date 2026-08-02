# release v0.25.0

## Summary
Ship the work accumulated since v0.24.0. Minor bump, **no breaking changes** — the engine
change is additive and the rest is `tools/trck-html`:

- **`trck deps` gutter** (`src/trck/render.py`) — lanes are shortened by sliding nodes along
  the order, so a dependency line spans fewer rows; the result is independent of the input id
  order (#xagqqgd).
- **`tools/trck-html` graph** — edges land on the arrowhead's base rather than its tip, the
  vertical gap opened to twice a node's height, rows ordered by barycentre with single-node
  relocations to cut crossings (#xagqqgd), layer-skipping edges routed through placeholder
  nodes (#6yptz6p), and a node's edges accented while the pointer is on it (#budhpcw).
- **`tools/trck-html` views** — facet boxes start checked, tree parents start collapsed and
  facets no longer force them open, font stacks hoisted and led with better families.
- **Tests** — `test_an_id_prefix_resolves` pins its ids instead of flaking on a random 2-char
  prefix collision (#cggyyxc).

## Implementation
Per the release process in `CLAUDE.md`:

1. Bump `__version__` in `src/trck/constants.py` from `0.24.0` to `0.25.0`
2. `python3 build.py`, then `python3 build.py --check` — exits 0, no diff
3. `python3 -m unittest discover -s tests` — zero failures
4. `./trck check && ./trck version`
5. Commit `./trck` with the source, tag `v0.25.0`, push, create the GitHub Release

## Acceptance criteria
- [ ] Version bumped in `src/trck/constants.py` and `./trck` regenerated from it
- [ ] `build.py --check` and the full suite pass
- [ ] Tag `v0.25.0` pushed and a GitHub Release published

## Notes
`#6ddksge` (id-collision reconcile) also closed in this range, as `superseded` — no code, so
it is not part of the notes.
