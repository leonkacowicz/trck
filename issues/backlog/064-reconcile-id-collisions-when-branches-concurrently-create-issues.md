# Reconcile id collisions when branches concurrently create issues

## Summary
When two branches each run `trck new`, both take `next_id` = `max(ids)+1` (trck:616) and
land the **same** integer id. On merge this clashes in three places at once, because the id
is the issue's identity everywhere:

- the index row key in `index.jsonl` (two objects with the same `id`),
- the filename prefix (`NNN-slug.md` — two files claiming `064-…`),
- every cross-reference (`parent`, `depends_on`, `successors`) that points at that id.

A plain text/JSONL merge can't fix this safely: to renumber one side you must rename its
file, rewrite its index row, **and** rewrite the references that belong to *that* side —
without disturbing the colliding references that legitimately belong to the other side.
The merge has no provenance, so it can't tell the two `64`s apart. That provenance problem
is the real engineering content of this issue.

This is an investigation: decide the approach before committing to an implementation.

## Acceptance criteria
- [ ] Decide the direction (see Notes): renumber-on-merge vs. collision-resistant identity.
- [ ] A documented, repeatable way to recover from a concurrent-creation collision without
      hand-editing `index.jsonl` or renaming files by hand.
- [ ] Cross-references (`parent`, `depends_on`, `successors`) and filename prefixes stay
      consistent after reconciliation.
- [ ] `trck check` passes on the reconciled result.
- [ ] Tests covering: two-branch same-id collision, a collision where the renumbered side is
      referenced by another issue, and a no-collision merge (no spurious renumbering).

## Notes
**Status: on hold pending #65.** Direction (2) is being pursued first as #65 (short random
ids). If that ships and no collision is ever observed in practice, this issue (the
renumber-on-merge driver) is likely YAGNI — revisit only if a real clash occurs.

Two distinct directions, to be decided here rather than assumed:

1. **Renumber-on-merge (keep integer ids).** A custom git merge driver on `index.jsonl`
   (registered via `.gitattributes`) plus a `trck reconcile` verb: detect duplicate ids,
   renumber the later-`created` side, and fix its filename + all inbound/outbound refs.
   Pro: keeps small human-friendly ids. Con: the provenance problem above — needs a reliable
   signal for which side owns a given id (created timestamp, branch, or a per-issue origin
   key), and must rewrite refs in both index rows and bodies.

2. **Collision-resistant identity (sidestep the clash).** Make the durable identity a
   ULID / random / content-hash key; keep the integer purely as a display handle. Concurrent
   creation then never collides, so there's nothing to reconcile. This is the git-bug / Fossil
   approach. Pro: eliminates the bug class instead of patching it. Con: larger change —
   cross-references and filenames must key off the stable id, touching most of the index I/O
   band.

Recommendation: capture the *problem* (this issue) and pick between (1) and (2) before
building. (2) is cleaner long-term but heavier; (1) preserves the current ergonomics at the
cost of a fiddly merge driver.

Relevant code: `next_id` (trck:616); index read/write and `NNN-slug.md` naming in the
index-I/O band; cross-ref fields `parent` / `depends_on` / `successors`.
