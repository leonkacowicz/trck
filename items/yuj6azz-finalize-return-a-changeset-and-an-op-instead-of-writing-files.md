# finalize: return a changeset and an Op instead of writing files

## Summary
`finalize` (`src/verbs/finalize.rs:19`) is the single write path, and it writes: index
`write_atomic`, summary `write_atomic`. A commit-building backend needs the same three
derivations but the *contents*, not the side effect.

Split it: derive as today, then return a changeset — index text, summary text, and the body
add/rename/delete the verb implies — plus a structured `Op` describing what the verb did
(`done <id> --resolution fixed`). A directory backend applies the changeset exactly as today.

## Acceptance criteria
- [x] `finalize` performs no I/O; a `DirBackend::apply` does, and the bytes on disk are identical to before for every mutating verb.
- [x] The changeset covers body creation (`new`) and the rename `set --slug`/`--title` implies, so no verb writes a file outside it.
- [x] Every mutating verb produces an `Op` value alongside its changeset.
- [x] Existing unit and conformance coverage of the write verbs passes unchanged.

## Notes
This is the task that makes #C9 (build a tree from the changeset) and #C10 (serialise the `Op` as a trailer) possible without touching the verbs again.

Landed in PR #34. `src/verbs/changeset.rs` holds `Edit`/`Changeset`/`Op`, `src/verbs/backend.rs`
holds the only filesystem write left in the path, and `set`'s body rename/retitle moved to
`src/verbs/edit/body.rs` — it has to capture where the body *was* before the row is edited,
because once `--slug` lands the row no longer says where its own file is.

Changeset paths are **tracker-relative**: an absolute path is a fact about one backend, and a
tracker in a git ref has a tree to address rather than a directory.

`Op` records **resolved** values, not what was typed — `new` pins the id it generated, `dep`
records the id a prefix resolved to, `mv` records canonical `mv <id> <status>` rather than the
`start`/`done` alias. An op echoing the input would replay into a different tracker as something
else. #93zhqbd inherits this when it serialises the trailer.

Two things deliberately left out: the merge drivers (`src/repo/drivers.rs`) still write directly,
because they run inside a merge where the working tree is not the merged result and they derive
no changeset from a user operation; and no conformance fixture was added, because nothing a user
or downstream tool can observe changed. Byte-identity was verified instead by driving every
mutating verb through this binary and the previous one and diffing the two trackers.
