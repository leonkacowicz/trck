# Use random alphanumeric ids to make concurrent creation collision-free

## Summary
Replace the sequential `next_id` = `max(ids)+1` scheme (trck:616) with short random
alphanumeric ids. The deterministic `max+1` is *why* two branches collide on concurrent
`new` (see #64): both compute the same number. A random id breaks that determinism — two
branches generate different ids, their `index.jsonl` rows union cleanly on merge, and because
the id is stable it never has to be rewritten, so cross-references (`parent`, `depends_on`,
`successors`) never break.

This is the lightweight form of "collision-resistant identity": no timestamp, no content hash
— just N random chars. It is "optimistic": collision is improbable, not impossible, but at a
tracker's real scale (thousands of issues) the birthday risk is negligible.

If this ships and proves sufficient, #64 (the renumber-on-merge driver) is likely **YAGNI** —
revisit it only if a real collision is ever observed.

## Acceptance criteria
- [ ] `new` generates a short random id (e.g. 7 base32 chars, ambiguous chars `0/O/1/l/I`
      removed) instead of `max+1`.
- [ ] Within-branch guard: regenerate if the drawn id collides with an id already visible in
      the index (cheap; only the unseen cross-branch tail stays optimistic).
- [ ] CLI accepts any **unambiguous id prefix** wherever an id is taken (`show`, `set`, `dep`,
      `mv`, `--parent`, `--depends`, …), git-short-hash style; ambiguous prefix is an error.
- [ ] Filenames key off the new id (`<id>-slug.md`); ordering in listings comes from `created`,
      not the id.
- [ ] Migration story for the existing integer ids wired into `parent` / `depends_on`
      (one-time renumber-all, or a documented mixed-scheme transition).
- [ ] `trck check` passes; tests cover generation, within-branch collision regeneration,
      prefix resolution (unique + ambiguous), and a two-branch merge with no clash.

## Notes
Alternatives considered and why short-random wins here:
- **ULID / timestamp-prefixed** — sortable *and* collision-resistant, but 26 chars is miserable
  to type on a CLI. Typeability beats id-sortability for trck since `created` already carries
  order.
- **Content hash** — stable but opaque and longer; no advantage over random for this purpose.

Tradeoff accepted: ids become less human-friendly than `64`. Prefix-matching + dropping
ambiguous characters recovers most of that.

Relationship to #64: these are **alternatives**. Ship this first; treat #64 as on-hold pending
whether a collision is ever actually seen in practice.

Relevant code: `next_id` (trck:616); index I/O and `NNN-slug.md` naming; id-taking args across
the command handlers.
