# ids: drop renumber and every trace of integer ids

## Summary
Integer ids were the first iteration. They were replaced because two branches running `trck new`
minted the same number and conflicted; random ids fixed that. What stayed behind is a migration
path — and the migration path is bigger than the verb.

`repo renumber` is ~45 lines. The support around it is not, and none of it is about renumbering:

| where | what |
|---|---|
| `index.py` | `legacy_id` field, its default, its validation, its resolution tier in `resolve_ref` |
| `index.py` | `issue_path` writes zero-padded `024-slug.md`; `file_id` un-pads it |
| `index.py` | `want_id` accepts a JSON integer as an id |
| `graph.py` | unique-prefix generation **reserves all-digit candidates** so "all-digit ⇔ legacy" holds |

That last one is the tell: a rule about what ids may look like, paid forever, to protect a
namespace nobody is still in. Rust would have to reimplement all of it.

Pre-1.0, and we are still working out what the tool should be. Anyone who genuinely needs to
convert can do it themselves — the repo ships a converter and a map file, and the mapping is
theirs to keep, not ours to carry.

## Acceptance criteria
- [ ] This repo's data migrated **first**, while `legacy_id` still exists: 152 `#NN` occurrences
      across 44 distinct numbers in issue bodies rewritten to real ids, and `legacy_id` stripped
      from the 70 rows carrying it. Same for the bundled example (all 35 rows).
- [ ] A committed map of this repo's old number → id, so `#24` in a commit message from before
      the change is still resolvable by a human reading history.
- [ ] `repo renumber` and its parser entry gone.
- [ ] `legacy_id` gone: field, default, validation, and the numeric tier of `resolve_ref`.
- [ ] The zero-padded filename convention gone from `issue_path` and `file_id`.
- [ ] `want_id` accepts strings only — an integer id in `index.jsonl` is a validation error.
- [ ] The all-digit reservation gone from prefix generation. An all-digit random id (`2345678`
      is a legal one) stops being a special case.
- [ ] A tracker that still has integer ids gets a **clean refusal naming the converter**, not a
      file-not-found. Refusing to touch it is fine; leaving it to fail obscurely is not.
- [ ] A standalone converter under `scripts/`, outside the engine, plus docs.
- [ ] No format bump. `legacy_id` becomes an unknown key, which `Issue.extra` round-trips
      verbatim, so an old tracker loses nothing by meeting a new engine.

## Notes
Kept deliberately: `__post_init__`'s `str()` coercion of `id`/`parent`/`depends_on`. It reads as
integer-id support but is type normalisation at the single construction choke point, and ~92 test
sites pass short int ids as a convenience. Rust's equivalent is that the field is a `String`.

**Not jq**, despite the plan. jq has no random source, so it cannot mint ids — a pure-jq converter
would have to be handed the ids to assign, which defeats the point. A stdlib Python script does the
whole job in ~40 lines and needs no engine.

Should close out `c2wadyd` (id type annotations still say `int`) as a side effect.
