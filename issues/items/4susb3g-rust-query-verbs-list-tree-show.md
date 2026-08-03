# rust: query verbs — list, tree, show

## Summary
The read surface people use most: the nested forest with its blocking notes and rollups, and the
single-issue view.

## Acceptance criteria
- [ ] `list`/`tree` with the full filter set, `--flat`, `--all`, `--sort`, `--show-field`,
      `--paths`, and subtree rooting.
- [ ] The dim `needs #NNN` / `blocks #NNN` annotations, at the same altitude as today.
- [ ] Parent rows with rolled-up percentages; settled subtrees hidden unless `--all`.
- [ ] `show` with metadata and body.
- [ ] Shortest-unique-id-prefix emphasis preserved.
- [ ] Passes the `xm6h2qn` fixtures.
