# new: --id to supply an issue id instead of generating one

## Summary
`trck new` mints a random id. `--id` supplies one instead. It is a small flag with two
unrelated customers, which is why it is its own issue rather than a line inside `5p738hu`.

**Conformance fixtures** (`5p738hu`) need it. Ids are random, so a fixture cannot know what
`new` will produce, and goldens either hardcode nothing or get patched afterwards. Two
alternatives were weighed and rejected:

- *Substitute `{alias}` into the golden.* Lossy in a way that matters: if a command mints two
  ids and emits them **swapped**, normalising by first appearance renames them to match and the
  fixture passes. A suite whose job is catching a second implementation getting this subtly
  wrong should not have that blind spot.
- *Seed the generator (`--seed`).* Makes id generation part of the conformance contract. Rust
  cannot reproduce CPython's `random` portably — `choice` goes through `_randbelow`, an
  implementation detail — so it would mean specifying a generator and implementing it twice, to
  pin values no user depends on. Ids would also depend on creation order, so inserting one setup
  line would rewrite every id in the golden.

**Import and recovery** need it too, and this is the reason it is a feature rather than a test
hook: moving issues in from another tracker while keeping their ids, restoring an issue deleted
by hand, scripted seeding. `--seed` had no honest non-test use; this does.

Named `--id`, not `--force-id`: "force" implies overriding something, and there is nothing to
override — you are supplying the value instead of generating it. `--force-id` also reads
adjacent to `--force`, which does something unrelated.

## Acceptance criteria
- [ ] `trck new --id ID`. A flag, not an env var: an env var applies to every `new` in a
      sequence, so a fixture creating two issues would have to set and unset it between calls.
- [ ] Refuses a duplicate — of an id in the index **and** of one only present on disk, since
      `gen_id` already guards against both.
- [ ] Refuses anything outside the alphabet or the wrong length. Without these two checks the
      flag is a way to corrupt a tracker by hand.
- [ ] The generated path is unchanged when the flag is absent: same `gen_id`, same guard.
- [ ] Documented as an import/recovery affordance, not as a test hook.

## Notes
Only `new` mints an id, so one flag covers everything. `repo renumber` used to mint many at once
but is gone (`dfe48ds`).

Deliberately **not** added to `set`: changing an existing issue's id would have to rewrite every
`parent`/`depends_on` pointing at it and rename its body file — a different feature, and one
nobody has asked for.
