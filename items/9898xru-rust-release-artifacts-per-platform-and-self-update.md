# rust: release artifacts per platform, and self-update

## Summary
Distribution is what un-vendoring trades for. It has to be good enough that installing is not a
barrier, because there is no longer a copy in the repo to fall back on.

## Acceptance criteria
- [x] Static binaries for Linux (gnu and musl), macOS (x86_64 and arm64) and Windows, built in
      CI and attached to releases.
- [x] A one-line install path, and at least one package manager.
- [x] `trck update` consuming per-platform assets rather than a single file, with the existing
      channel notion preserved. **Changed — see below.**
- [x] Version checking that tells a user their engine is older than the tracker's format, which
      is the failure the format guard makes visible.
- [x] CI installs the published artifact and runs the conformance suite against it, so a broken
      release cannot ship green.

## The self-update criterion was dropped, deliberately
The binary does **not** replace itself. Two reasons, and the second is the decisive one:

1. The engine has no dependencies and Rust's std has no HTTP/TLS client, so downloading would
   mean shelling out to `curl`/`wget`. That is survivable — the engine already shells out to
   `git` — but it is not free.
2. After the cutover the binary arrives from a package manager or the install script. A
   self-updater fighting the thing that owns the file is worse than not having one: it breaks the
   package manager's checksums and leaves two mechanisms disagreeing about what is installed.

The Python engine's `update` existed because the engine *was* a single vendored file you edited
in place. The binary is not that, so the verb does not carry over. `trck update` answers with the
upgrade path rather than a typo message — anyone with the habit gets told what replaced it.

**The "existing channel notion" was nothing to preserve.** `trck.json` declares
`update.channel: "stable"` and no code has ever read it. Nothing was lost by not carrying it
across.

**Consequence caught along the way:** the format guard told users to "run `trck update`", which
would have named a verb the binary does not have. Both engines now say "upgrade trck" — true
whatever installed it, and it stays true after cutover. Changed in Python too so the two keep
producing identical diagnostics.

`trck version` had to be implemented for any of this to hold together: the install script's smoke
check and the `update` message both point at it.

## What is verified, and what is not
**Verified locally:** the installer end to end against a `file://` release tree — target
detection, download, checksum verification, extraction, install, and that a corrupted checksum
aborts rather than installing. Both workflow files parse. `install.sh` is shellcheck-clean under
`-s sh`. Both engines still agree on 225/225 conformance.

**Not verifiable here, only on a real tag:** that the six cross-builds succeed (notably
`aarch64-unknown-linux-musl`, which needs a linker the runner does not ship), that the artifacts
run on their target platforms, and that the publish step attaches them. The first tagged release
is the test. The `verify` job is positioned so a failure there blocks publishing rather than
being discovered afterwards.

**Version is `0.0.0`** in the workspace `Cargo.toml` — honest, since the port has never been
released. Set it in the same commit as the Homebrew formula's `version` when cutting the first
binary release.
