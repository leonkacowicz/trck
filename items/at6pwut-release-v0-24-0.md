# release v0.24.0

## Summary
Ship the work accumulated since v0.23.0. Minor bump, **no breaking changes** — everything in
this range is additive:

- `trck diff` — a VCS-agnostic change model plus a git convenience layer (revision specs,
  `HEAD` default). Partial: the source seam, the change model and the git layer landed; the
  rendering layouts (`--stat`, `--flat`, `-v`, epic rollup) are still open under #u5fc5vm.
- Git integration — row-wise 3-way merge of `index.jsonl` keyed by id, `trck repo
  merge-index` / `merge-summary` drivers, and `trck repo setup-git` to register them per clone.
- Index safety — duplicate ids in `index.jsonl` are refused; a non-terminal issue carrying
  `resolution` or `closed` fails validation.
- `tools/trck-html` — ready view ranked by the demand cone, ready-leaf markers in the tree
  view, per-view status/priority checkbox facets.

## Implementation
Per the release process in `CLAUDE.md`:

1. Bump `__version__` in `src/trck/constants.py` from `0.23.0` to `0.24.0`
2. `python3 build.py`, then `python3 build.py --check` — exits 0, no diff
3. `python3 -m unittest discover -s tests` — 844 tests, zero failures
4. `./trck check && ./trck version` → `OK — …` then `0.24.0`
5. Commit `./trck` with the source, tag `v0.24.0`, push, create the GitHub Release

## Acceptance criteria
- [ ] Version bumped in `src/trck/constants.py` and `./trck` regenerated from it
- [ ] `build.py --check` and the full suite pass
- [ ] Tag `v0.24.0` pushed and a GitHub Release published

## Notes
One flaky test surfaced during the release run and is filed separately as #cggyyxc — it is a
test-fixture bug (random 2-char id prefix collision), not an engine defect, and does not gate
this release.
