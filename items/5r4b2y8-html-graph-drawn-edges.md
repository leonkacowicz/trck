## Summary

Pure functions in `assets/app.js` that turn the model into the edge set `deps` draws, over a
given set of visible ids: authored, containment and inherited edges (with the
`carried_above` suppression), each carrying its kind, and a transitive reduction that drops
an edge only in favour of a path inside the same set. Ports of `gutter::drawn_edges` and
`gutter::reduce::transitive_reduction`; must terminate on a malformed cycle, as the Rust does.

## Acceptance criteria

- [ ] `tests/app_js.rs` cases mirroring the Rust ones: an epic waits on its children; an
      ancestor's dependency is drawn under a child only when no ancestor in between is
      visible; a reduced edge is dropped only when the path is drawn; a cycle terminates.
