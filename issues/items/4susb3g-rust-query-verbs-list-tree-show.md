# rust: query verbs — list, tree, show

## Summary
The read surface people use most: the nested forest with its blocking notes and rollups, and the
single-issue view.

## Acceptance criteria
- [x] `list`/`tree` with the full filter set, `--flat`, `--all`, `--sort`, `--show-field`,
      `--paths`, and subtree rooting.
- [x] The dim `needs #NNN` / `blocks #NNN` annotations, at the same altitude as today.
- [x] Parent rows with rolled-up percentages; settled subtrees hidden unless `--all`.
- [x] `show` with metadata and body.
- [x] Shortest-unique-id-prefix emphasis preserved.
- [x] Passes the fixtures that exist — 10 of 11, the one failure being `deps` (`bdmgj7r`).
      `xm6h2qn`'s conversion has not run, so this could not be met literally; two new
      fixtures cover `list` and `show` and the CI floor moved 8 -> 10. The real evidence is
      the differential sweep below.

## Landed
`cb1c6de`. `render.rs` and `query.rs`.

**Verified by differential sweep against the Python engine over both real trackers** — 59
invocations covering the nested forest, `--flat`, `--all`, every filter, every sort
including `field:NAME`, `--paths`, `--show-field`, subtree rooting, and `show` on five
issues — byte-identical, exit codes included.

Two things the sweep caught that hand-written tests would not have:

**`show` aligns its key column over every *candidate* key**, not only the ones carrying a
value. An issue with no `manual_status` still lines up with one that has it, so two `show`
outputs sit in the same column. Easy to get wrong, invisible until you diff.

**List-valued fields print as Python list literals.** That looked like an artifact of the
first implementation; it is contract, because the suite compares stdout literally. So
`python_list` moved to `render.rs` and is shared by `show`, `label` and `dep`.

**An unrecognised flag is now refused and exits 2.** Silently dropping one is the worst
outcome available: `list --stauts done` would list everything and read as a successful
filter. Exit 2 rather than 1 is argparse's convention and a real distinction — a script can
tell "you called me wrong" from "what you asked for failed".

One divergence stands: that refusal exits 2 in both engines but the message differs,
because Python prints argparse's usage block. Reproducing it would be pinning argparse
rather than trck.
