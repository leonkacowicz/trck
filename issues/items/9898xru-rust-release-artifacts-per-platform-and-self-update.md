# rust: release artifacts per platform, and self-update

## Summary
Distribution is what un-vendoring trades for. It has to be good enough that installing is not a
barrier, because there is no longer a copy in the repo to fall back on.

## Acceptance criteria
- [ ] Static binaries for Linux (gnu and musl), macOS (x86_64 and arm64) and Windows, built in
      CI and attached to releases.
- [ ] A one-line install path, and at least one package manager.
- [ ] `trck update` consuming per-platform assets rather than a single file, with the existing
      channel notion preserved.
- [ ] Version checking that tells a user their engine is older than the tracker's format, which
      is the failure the format guard makes visible.
- [ ] CI installs the published artifact and runs the conformance suite against it, so a broken
      release cannot ship green.
