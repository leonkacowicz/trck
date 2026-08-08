# Contributing

```bash
cargo build --release
cargo test --all                                          # the engine's own tests
python3 conformance/run.py --bin target/release/trck      # the executable specification
python3 -m unittest discover -s scripts/tests             # the helper scripts
```

## The engine

The engine is `src/` — one package at the repo root, no workspace — and it takes **no
dependencies**: the binary is a single artifact a repository depends on for years, and every
dependency is a future reason it stops building. The lints deny `unsafe`, `unwrap`, `expect`
and `panic`: a malformed tracker must produce a diagnostic, never a stack trace.
`cargo fmt --all --check` and `cargo clippy --all-targets --all-features -- -D warnings` both
gate CI, and all three suites above run there.

## The three suites

`conformance/` is the executable specification, and it is the one worth understanding first.
It **execs** a binary rather than importing anything, so it describes behaviour instead of
implementation: a fixture is a starting tracker, one command, and what that command should
print. Anything a user or a downstream tool would notice belongs there; internals stay in
unit tests. The release workflow installs the built artifact and runs the suite against it,
so a build that cannot pass its own spec never becomes a download.

`cargo test --all` covers the engine's internals, plus two cases nothing else would catch:
`tests/app_js.rs` lifts pure functions out of the compiled-in `assets/app.js` and runs them
under `node` (skipped when node is absent), and `tests/broken_pipe.rs` closes a reader on a
running verb.

`scripts/tests` covers the helper scripts — the installer, a timestamp backfill, an id
converter, and the CI path classifier. None is part of the engine, which is why they are a
separate suite rather than something gating every engine change.

Add a test for every change, in whichever of the three it belongs to.

## What CI runs, and when

A pull request that touches only `issues/`, `docs/` or repository-root markdown cannot affect
the engine, so the build matrix, the conformance suite, the quality ratchet and the
helper-script suite are skipped for it; `trck check` still runs. Anything else runs everything,
as does every push to `main`.

`scripts/ci_changed.py` is that decision, and it is an **allowlist** — every path is code
unless it is in one of those three places. Markdown is not a safe signal in this repository:
`assets/` holds compiled-in templates, `conformance/` holds fixtures, `examples/` holds a
tracker the screenshots are generated from. Uncertainty resolves to running everything, because
the failure is silent in the other direction: a rule that wrongly says "skippable" does not
turn a check red, it makes the checks green by never running them. Widening the rule means
adding a case to `scripts/tests/test_ci_changed.py` first.

The skip is expressed as an `if:` on each job rather than `paths-ignore` on the workflow.
Merging is gated on named checks, and a workflow filtered out by paths never reports them — the
pull request would wait indefinitely for a check that is never coming. A job skipped by an
`if:` reports as skipped, which branch protection counts as passing.

**Except a matrix job.** `rust` carries no `if:`, because a matrix job skipped as a whole
reports once under the bare name `rust` and never produces `rust (ubuntu-latest)` — the check
merging is actually gated on, and the same indefinite wait `paths-ignore` would have caused. So
it always runs; what shrinks is the matrix (`changes` publishes it: three platforms, or just
`ubuntu-latest`) and what its steps do. Build and `trck check` run either way, which is why
there is no separate tracker job.

## The quality ratchet

`quality-report.json` is a committed snapshot of structural metrics — function length,
cognitive and cyclomatic complexity, argument counts, file size. CI runs
[ratchet](https://github.com/leonkacowicz/ratchet) over it twice: `check` fails if the report
no longer describes the code, and `compare` fails if any metric got worse than the baseline.
Existing debt is grandfathered and may only shrink, so a change under `src/` needs
`ratchet generate` and the regenerated report staged with it. If a threshold itself needs to
move, that is its own commit: ratchet refuses a threshold edit in the same change as a new
violation.

Enable the pre-commit guard once per clone with `git config core.hooksPath scripts/hooks`.

## Dogfooding

This repo **self-hosts** its own issues under `./issues/` — browse them to see `trck` tracking
its own roadmap. Use the built binary for bookkeeping and hand-edit only an issue's markdown
body; `index.jsonl` and `SUMMARY.md` are generated.

The README screenshots are regenerated from the bundled example tracker with
`python3 docs/gen-screenshots.py`, which writes the SVGs under `docs/img/`.

## Releasing

Bump `version` in `Cargo.toml` and in `packaging/homebrew/trck.rb` in one commit, then tag
`vX.Y.Z`. The release workflow cross-builds every target, verifies the artifact against
`conformance/`, and only then publishes. It skips — rather than fails — when the tag does not
match the `Cargo.toml` version, so a tag cut for some other purpose leaves no red run behind;
if a release you expected never appears, read the guard's run summary first.
