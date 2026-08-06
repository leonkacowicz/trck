# release.yml fires on any v* tag, including a Python one

## Summary
`.github/workflows/release.yml` triggers on `push: tags: ['v*']`, and it builds the *Rust*
binary. That was unambiguous while the only tags being cut were Rust ones. It stops being
unambiguous the moment the final Python release is tagged: `v0.26.0` would cross-build six
targets from a workspace `Cargo.toml` that still says `version = "0.0.0"`, run the conformance
suite against them, and publish a release whose tag and whose binaries disagree about what
version they are.

The tag namespace is shared between the two engines by history, and only one of them is what
`release.yml` knows how to build.

## Acceptance criteria
- [ ] A tag whose version does not match the workspace `Cargo.toml` does not produce a release —
      the job either skips cleanly or fails loudly, but it does not publish.
- [ ] The check runs before the six-target matrix, so a mismatched tag costs seconds, not a
      full cross-build.
- [ ] Whatever the rule is, it is stated in `CLAUDE.md` beside the release instructions, which
      currently say to bump `Cargo.toml` and tag as though nothing else could match.
