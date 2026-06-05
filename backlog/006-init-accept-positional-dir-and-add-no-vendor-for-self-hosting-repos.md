# init: accept positional dir and add --no-vendor for self-hosting repos

## Summary
`trck init issues` (positional) is the natural form but currently errors — `init` only accepts `--dir`. Also add `--no-vendor` so the canonical/self-hosting repo doesn't get a duplicate engine copy (during dogfooding the vendored `issues/trck` had to be removed by hand, per spec §9.1).

## Acceptance criteria
- [ ] `trck init [dir]` positional works (alongside or instead of `--dir`)
- [ ] `--no-vendor` skips copying the engine into the tracker dir
- [ ] docs updated

## Notes
Surfaced during dogfooding the A+B build.
