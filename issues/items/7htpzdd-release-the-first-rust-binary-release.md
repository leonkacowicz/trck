# release: the first Rust binary release

## Summary
The release machinery from `#9898xru` is built and has never been fired: the workspace
`Cargo.toml` still says `version = "0.0.0"` and no tag has ever produced a binary. That is fine
while the port is measured against fixtures, and not fine the moment the cutover tells people to
install a binary — the answer to "how do I get trck now" has to resolve to something that
exists. `scripts/install.sh` and `packaging/homebrew/trck.rb` both fetch by version.

The Python engine's last release is `v0.25.0`, and the tag namespace is shared, so the number
this takes is a decision rather than an increment. Two coherent readings:

- **Continue the series.** The next tag after the final Python release. One project, one line of
  versions, and `trck version` keeps counting up across the engine swap.
- **Start at 1.0.0.** The binary is the product now; the 0.x series was the single-file script.
  A clean break, at the cost of the tag order no longer matching release order.

Whichever it is, the release workflow already installs the musl artifact and runs the
conformance suite against it before publishing, so a build that cannot pass its own spec never
becomes a download. The first run of that path is itself the thing being tested here.

## Acceptance criteria
- [ ] A version chosen, set in the workspace `Cargo.toml` and in `packaging/homebrew/trck.rb`
      in the same commit.
- [ ] A tag pushed, and `.github/workflows/release.yml` observed end to end: six targets
      cross-built, the musl artifact installed and run against `conformance/`, assets uploaded.
- [ ] `scripts/install.sh` verified against the real release — download, checksum, install.
- [ ] The Homebrew formula verified against the real tarball.
