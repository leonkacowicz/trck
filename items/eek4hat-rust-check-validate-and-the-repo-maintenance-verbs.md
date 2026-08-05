# rust: check, validate and the repo maintenance verbs

## Summary
`check` is the contract enforcer — the pre-commit hook runs it — plus the rare-but-essential
`repo` verbs, including the merge drivers git itself invokes.

## Acceptance criteria

Narrowed on starting. The git-integration verbs moved to `qktc8z7`: they shell out to git, write
into `.git/config`, and — for the merge drivers — run *inside* a merge, where the contract is
about behaviour given three inputs mid-operation. Testing that means driving real merges, a
harness nothing else in the port needs. `check` reads a tracker and reports; bundling the two
made an issue that could not be finished in one go.

`repo renumber` is struck outright: integer ids were dropped (`dfe48ds`) and the converter lives
in `scripts/renumber.py`, outside the engine.

- [x] `check`: every current validation, same messages, nonzero exit on error.
- [x] `summary` and `repo normalize`.
- [x] Validation reported after every mutation, as `finalize` does today — a verb that leaves
      the tracker inconsistent still succeeds, but says so.

## Landed
`8b27452`. `validate.rs`, plus `check`, `summary` and the post-mutation report.

**A clean tracker exercises almost none of this**, which is the trap: `check` against both real
trackers agreed immediately and proved nothing. The verification is 22 deliberately-broken
trackers, one per error class — missing file, orphan file, slug mismatch, bad slug, unknown
status, bad priority, negative points, points on a parent, bad resolution, a non-terminal row
carrying `closed` or `resolution`, bad review URL, bad and non-string custom fields, dangling
parent, dangling dependency, parent cycle, authored cycle, a cycle implied through the
hierarchy, a parent whose status is not its rollup, a terminal issue depending on an open one,
and vestigial config keys. All 22 byte-identical, exit codes included.

**The 22nd found a real bug.** `scan_files` split filenames on the first dash without applying
the filename pattern, so a README or scratch note in `items/` would have been mistaken for an
issue and reported as one missing its index row. Only the bad-slug case exposed it, and only
because the fixture's *filename* was malformed rather than its index row.

That is the second time this session a sweep over real data passed while testing nothing —
after the `ready` demand annotation. The pattern is worth naming: real data exercises the
paths that are working, and error paths are by construction the ones real data avoids.
