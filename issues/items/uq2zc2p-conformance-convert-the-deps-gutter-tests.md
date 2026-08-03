# conformance: convert the deps gutter tests

## Summary
The gutter is printed to a terminal, so it is product, not implementation — if the port draws
`●─┼─╯` differently, users notice immediately. The existing tests already assert on the drawn
strings; they just do it from Python.

## Acceptance criteria
- [ ] Golden gutters for the whole-graph view and the id-scoped cones (`--requires`, `--blocks`,
      `--full`).
- [ ] Row order, which decides the order lines appear in and is equally visible.
- [ ] Lane assignment across the cases that shaped it: fan-out, merge, bridges, component
      separation, transitive reduction, containment edges, inherited edges.
- [ ] Done-filtering: `--omit-done`, `--include-done-chains`, and the whole-graph default.
- [ ] The lane-shortening pass, by its visible result rather than its cost function.

## Notes
Golden gutters are wide box-drawing strings; the runner's diff output needs to stay readable
when one changes.
