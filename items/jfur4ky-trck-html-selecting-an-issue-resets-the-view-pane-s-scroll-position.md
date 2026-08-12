# trck-html: selecting an issue resets the view pane's scroll position

## Summary
Clicking a node in the graph tab scrolls the graph back to the top-left. Scroll down to a
component, click a node to read its body in the detail pane, and the pane you were reading
jumps away from under the pointer — the one thing a click should never do.

The cause is not specific to the graph. `select()` calls `renderActiveView()`, and every view
renderer empties its container (`box.textContent = ''`) and rebuilds it. The container *is* the
scrolling element (`.graph`, `.list`, `.tree`, `.board` are all `overflow: auto`), so wiping its
children drops `scrollTop`/`scrollLeft` to zero. The graph is where it hurts most because it is
the largest canvas and the one most often scrolled far from the origin, but list, tree and board
have the same behaviour.

Selection changes only which node carries the `sel` class; it changes no layout and no geometry.
So the offsets that were valid before the rebuild are still valid after it, and holding them
across the rebuild restores the view exactly.

## Acceptance criteria
- [ ] Selecting an issue in the graph tab leaves the graph's scroll position unchanged.
- [ ] The same holds for the list, tree, board and ready tabs.
- [ ] The behaviour is covered by a test in `tests/app_js.rs` (a lifted helper exercised under
      node against a scroll-container stand-in), so a future refactor of `select()` cannot
      silently reintroduce it.

## Notes
- `assets/app.js`: `select()` (~line 331), `renderActiveView()`, `renderGraph()`.
- `assets/app.css`: `.graph`, `.list`, `.tree`, `.board` all scroll in place at `height: 100%`.
- The pane element ids match the `state.view` values one-for-one (`list`, `graph`, `tree`,
  `board`, `ready`), so the active scroll container is `$('#' + state.view)`.
- Board columns (`.bcards`) scroll independently inside the board; restoring the board's own
  offsets does not restore those. Out of scope unless it turns out to bite.
