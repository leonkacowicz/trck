# rust: the git-integration verbs — hooks, merge drivers, setup

## Summary
Split out of `eek4hat`, which bundled them with `check`. They are a different kind of thing:
`check` reads a tracker and reports, while these shell out to git, write into `.git/config`, and
— in the merge drivers' case — **run inside a merge**, where the working tree is not yet the
merged result.

That last point is why they deserve their own issue. `merge-index`'s contract is not "produces
the right file" but "produces the right file *given three inputs git hands it mid-operation*",
and its behaviour under conflict is part of that contract. Testing it means driving real merges,
which is a harness the rest of the port does not need.

- `repo install-hook` — write a pre-commit hook running `trck check`.
- `repo setup-git` — write `.gitattributes` and register the drivers in *this clone's*
  `.git/config`. Git shares the attributes file but never the driver commands, because that
  would make cloning remote code execution — so this is per-clone, and every clone must run it.
- `repo merge-index` / `merge-summary` — the drivers git invokes. Row-wise, keyed on id.
- `repo migrate-layout` — the pre-0.23 status-folder migration, and whatever `rbast9r` lands.

## Acceptance criteria
- [ ] `install-hook` and `setup-git`, writing the same files as the Python engine.
- [ ] `merge-index` merging row-wise on id, with the same conflict behaviour — including the
      `(status, closed, resolution)` tuple resolved as one unit, which is exactly what a
      field-wise merge gets wrong and what `check` then reports.
- [ ] `merge-summary`, which regenerates rather than merges.
- [ ] `migrate-layout`, and the refusal every verb gives an unmigrated tracker.
- [ ] Tests that drive a real git merge, not a simulated one. The failure mode this code exists
      to prevent only appears when git is the one calling.

## Notes
`repo renumber` is **not** in scope: integer ids were dropped outright (`dfe48ds`) and the
converter lives in `scripts/renumber.py`, deliberately outside the engine.
