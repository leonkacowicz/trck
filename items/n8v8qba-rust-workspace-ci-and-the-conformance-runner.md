# rust: workspace, CI, and the conformance runner

## Summary
The first Rust that gets written is the fixture runner, not the engine — so the suite is red on
day one and turns green as the port lands. That inverts the usual rewrite failure mode, where
correctness is assessed at the end by reading code.

## Acceptance criteria

Rewritten on starting. The plan wanted **a second runner, in Rust**, reading the same fixtures
"with the same alias substitution and date normalisation". Both halves are obsolete:

- There is no alias substitution — `jpash72` replaced it with chosen ids — and date
  normalisation is `TRCK_NOW`, which the existing runner sets.
- More importantly, `conformance/run.py` already **execs** `$TRCK_BIN`. It was built
  binary-agnostic precisely so one runner serves both engines. A second implementation of the
  harness would need to agree with the first about fixture semantics, comparison and
  normalisation — reintroducing, in the thing that judges correctness, exactly the divergence
  risk the suite exists to catch. One runner, two binaries.

- [x] Cargo workspace at the repo root: a `trck` binary crate, lints and formatting enforced.
- [x] CI builds and tests on Linux, macOS and Windows.
- [x] CI runs the conformance suite against the Rust binary and **reports the pass rate**, so
      progress through the port is a number rather than a feeling.
- [x] That job does not block on failures it is expected to have — but it does not silently
      pass either. A floor that only moves up: the build fails if fewer fixtures pass than the
      committed baseline, so the port cannot regress.
- [x] A differential mode in `run.py`: run a fixture against two binaries and diff their
      output against each other rather than against a golden. This is the oracle for the whole
      port — it answers "do these agree" for cases nobody wrote a fixture for.
- [x] The Rust binary exists and does nothing yet. That is the point: the suite is red on day
      one and goes green as the port lands, which inverts the usual rewrite failure mode where
      correctness is assessed at the end by reading code.

## Landed
`7ec484a`. Workspace at the repo root, `crates/trck` an empty binary, CI matrix on all three
platforms, conformance reporting 0/4 against it.

**The Rust runner was struck**, reasoning above. What replaced it is two flags on the existing
runner:

`--min-pass N` — a floor that only moves up. The alternative shapes are both useless: a job
that must pass every fixture is red for months and gets ignored, and a job marked
`continue-on-error` is green and means nothing. With a floor, a regression fails the build and
raising it is one visible commit per step forward.

`--compare-bin` — differential mode, the oracle. Diffs two binaries against each other rather
than against goldens, so a disagreement surfaces for cases nobody has written a fixture for
yet. This is what makes the port verifiable beyond the fixtures that happen to exist.

Conformance runs on Linux only. The fixtures compare output literally and path rendering
differs on Windows; cross-platform correctness is what the build and test matrix is for.

Lints deny `unsafe`, `unwrap`, `expect` and `panic`. The engine reads files and exits with a
status — those four are precisely what turns a malformed tracker into a stack trace instead of
a diagnostic, and it is much cheaper to forbid them now than to unpick them later.
