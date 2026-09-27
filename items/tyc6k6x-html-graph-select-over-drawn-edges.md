## Summary

Switch `renderGraph`'s node set to the drawn edges: the unfiltered view takes the components
holding at least one authored edge, whole (`gutter::overview_ids`); filter-bar seeds take any
matching issue on a drawn edge and grow along `Graph::dependency_line`; done-chain hiding
computes its components over drawn edges. Layout then runs on the reduced set.

## Acceptance criteria

- [ ] An epic with no authored edges of its own appears when its component has one.
- [ ] The default view matches `trck deps --omit-done` on the same tracker.
