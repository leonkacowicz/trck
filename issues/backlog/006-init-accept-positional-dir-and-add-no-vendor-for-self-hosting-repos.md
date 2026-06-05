# init: accept positional dir and add --no-vendor for self-hosting repos

## Summary
`trck init issues` (positional) is the natural form but currently errors — `init` only accepts `--dir`. Also add `--no-vendor` so the canonical/self-hosting repo doesn't get a duplicate engine copy (during dogfooding the vendored `issues/trck` had to be removed by hand, per spec §9.1).

## Acceptance criteria
- [ ] `trck init [dir]` positional works (alongside or instead of `--dir`)
- [ ] `--no-vendor` skips copying the engine into the tracker dir
- [ ] `init` fails cleanly (a `die` message, not a `SameFileError` traceback) when `--dir` targets the running engine's own directory
- [ ] docs updated

## Notes
Surfaced during dogfooding the A+B build. The `SameFileError` traceback on a
self-targeting `init` was flagged in the final v0.1.0 review (no data risk —
`shutil.copyfile` checks identity before writing — just a UX wart).
