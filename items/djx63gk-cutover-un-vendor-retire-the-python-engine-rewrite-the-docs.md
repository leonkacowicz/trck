# cutover: un-vendor, retire the Python engine, rewrite the docs

## Summary
The last step, and the irreversible one. Only worth taking when the conformance suite passes
fully against the Rust binary and the differential runner finds no divergence on this repo's own
tracker.

## Acceptance criteria
- [x] `trck init` stops vendoring; the vendored-engine resolution path goes — including the
      branch that resolved a tracker from the running binary's own directory, which only ever
      made sense for an engine committed beside its data.
- [x] `src/trck/`, `build.py`, the amalgamation and its MANIFEST discipline all removed, along
      with `./trck` itself and the 44 test modules that imported it.
- [x] The pre-commit hook, CI, `conformance/run.py`, `docs/gen-screenshots.py` and
      `scripts/tests/` all point at the binary — the one in `target/`, so the engine under
      change is the one that answers rather than whatever happens to be installed.
- [x] **`trck check` survives the deletion.** Carried over to the `rust` job, where a built
      binary already exists. See `#k6g7kvf` for the same check as something consumers can use.
- [x] The 706 deleted tests accounted for. What remains: 175 engine tests, 28 helper-script
      tests, and 227 conformance fixtures. Most of what went tested things that no longer
      exist — an amalgamation build, a self-updater, a second implementation. Three areas lost
      real coverage and are filed as `#38qfknm`: `install-hook` (now untested entirely),
      `setup-git`'s `.git/config` half, and `diff` across git revisions. All three are the
      parts that need a real git repository, which is what made them awkward as fixtures.
- [x] README, `CLAUDE.md`, `issues/CLAUDE.md`, the skill and `conformance/README.md` rewritten.
      They describe what trck is, with no account of what it used to be.
- [x] A documented answer for a contributor without trck installed: the README leads with the
      install script, and in this repository `cargo build --release` produces the engine every
      harness already looks for.
- [x] A final release of the retired engine, tagged `v0.25.1` and published as a pre-release so
      it cannot become `/releases/latest` and misdirect the installer. Its `update` verb answers
      with the migration path instead of fetching a file that no longer exists.
