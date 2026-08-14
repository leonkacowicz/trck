# finalize: return a changeset and an Op instead of writing files

## Summary
`finalize` (`src/verbs/finalize.rs:19`) is the single write path, and it writes: index
`write_atomic`, summary `write_atomic`. A commit-building backend needs the same three
derivations but the *contents*, not the side effect.

Split it: derive as today, then return a changeset — index text, summary text, and the body
add/rename/delete the verb implies — plus a structured `Op` describing what the verb did
(`done <id> --resolution fixed`). A directory backend applies the changeset exactly as today.

## Acceptance criteria
- [ ] `finalize` performs no I/O; a `DirBackend::apply` does, and the bytes on disk are identical to before for every mutating verb.
- [ ] The changeset covers body creation (`new`) and the rename `set --slug`/`--title` implies, so no verb writes a file outside it.
- [ ] Every mutating verb produces an `Op` value alongside its changeset.
- [ ] Existing unit and conformance coverage of the write verbs passes unchanged.

## Notes
This is the task that makes #C9 (build a tree from the changeset) and #C10 (serialise the `Op` as a trailer) possible without touching the verbs again.
