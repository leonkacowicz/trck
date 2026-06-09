# Eliminate unnecessary index double-reads

## Summary
A couple of code paths load the index from disk more than once for a single operation:

- `cmd_check` (`trck:1297-1307`): `validate(ctx)` already does `load_index` internally,
  then the success line calls `load_index(ctx)` **again** just to count issues
  (`f"OK — {len(load_index(ctx))} issues …"`).
- `finalize` (`trck:863-867`): writes the index via `save_index`, then calls
  `validate(ctx)`, which **re-reads** the just-written index (and re-scans files) from
  disk. The file re-scan is the deliberate "validate the persisted state" check, but
  re-parsing the index we hold in memory is avoidable work.

These are correctness-neutral; the goal is to stop paying for redundant reads/parses on
every mutation and on `check`.

## Acceptance criteria
- [ ] `cmd_check` reports the issue count without a second `load_index` call (have
      `validate` return the count, or count from data it already produced).
- [ ] `finalize`/`validate` avoid re-parsing the index they just wrote/built where
      practical, **without** weakening the filesystem-vs-index consistency check
      (the on-disk file scan must still happen — that's the whole point of validating
      after a write).
- [ ] No change to validation results, warnings, or exit codes; tests pass.

## Notes
- Be careful: `finalize` deliberately validates the *persisted* state so that an index
  that fails to round-trip is caught. Reuse the in-memory rows for parsing, but keep
  `scan_files` reading the actual folders.
- Smallest viable change is fine — e.g. let `validate` accept already-loaded rows, or
  return the count for `cmd_check`. Don't restructure the validation pipeline wholesale.
