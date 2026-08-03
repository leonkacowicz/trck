# rust: workspace, CI, and the conformance runner

## Summary
The first Rust that gets written is the fixture runner, not the engine — so the suite is red on
day one and turns green as the port lands. That inverts the usual rewrite failure mode, where
correctness is assessed at the end by reading code.

## Acceptance criteria
- [ ] Cargo workspace, lints, formatting, and CI running on Linux, macOS and Windows.
- [ ] A runner reading the same fixture directories as the Python one, with the same alias
      substitution and date normalisation.
- [ ] A differential mode: run both engines over one tracker and diff stdout, `index.jsonl` and
      `SUMMARY.md`. This is the oracle for the whole port.
- [ ] CI reports the fixture pass rate, so progress through the port is a number.
