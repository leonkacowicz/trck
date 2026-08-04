# list: nested forest default (--flat, positional id, dimmed ancestors, tree alias)

## Summary

Phase 4 of #qapvxpz — the real work, written once on top of the Graph (#uqtjp9p) and the shared
renderer (#33frt7s). Implements the merged browse verb per #qapvxpz's "Design (decided)".

- `list` renders a nested forest by default; `--flat` recovers today's flat, globally
  sorted output via the same renderer with an empty prefix.
- `list <id>` (optional positional) roots the forest at one issue's subtree.
- Filtering shows matched nodes plus their ancestor spine; non-matching ancestors render
  dimmed. `--sort` orders siblings recursively (a child never escapes its parent).
- `tree` becomes a plain alias for `list` (carrying the positional id).
- `block_annotations` is rewired to `(g, r)` and works through both flat and nested paths.

## Acceptance criteria
- [ ] `trck list` prints a nested forest: every issue, children under parents.
- [ ] `trck list --flat` reproduces today's flat, globally sorted output (columns unchanged).
- [ ] Filtering = matched nodes + dimmed ancestor spine; all current filters keep meaning.
- [ ] `--sort` reorders siblings only, recursively.
- [ ] `trck list <id>` roots the forest at that subtree.
- [ ] `trck tree` / `trck tree <id>` alias to `list` / `list <id>`.
- [ ] `ready`/`next` unchanged; dangling-parent and dep cycles render without crashing.

## Notes

Depends on #33frt7s (renderer) and #uqtjp9p (traversal). Help/README and acceptance tests are
split into #sdun4vt.
