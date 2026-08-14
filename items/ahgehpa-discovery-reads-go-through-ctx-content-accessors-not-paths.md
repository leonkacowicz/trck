# discovery: reads go through Ctx content accessors, not paths

## Summary
`Ctx` hands out paths — `index_path()`, `items_dir()`, `summary_path()`
(`src/discovery.rs:145-160`) — and every read verb opens them itself. A tracker that lives in a
git ref has no paths to hand out, so there is no seam to slot a second source into.

Turn the accessors into content: `read_index()`, `read_body(&Issue)`, `read_config()`. A
directory-backed `Ctx` implements them with the same file reads it does today.

## Acceptance criteria
- [ ] All nine read verbs (`list`, `tree`, `ready`, `next`, `deps`, `show`, `check`, `summary`, `html`) obtain content through `Ctx`, not by joining paths themselves.
- [ ] Byte-identical output before and after; the conformance suite is untouched and still passes at the current ratchet.
- [ ] `path`/`which` still answer with a filesystem path for a directory-backed tracker.

## Notes
Pure refactor. The point is that #B6 becomes an implementation of an existing trait/enum rather than a rewrite of the read verbs.
