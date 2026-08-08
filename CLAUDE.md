# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`trck` is an in-repo issue tracker: one markdown file per issue under a tracker directory, all
metadata in `index.jsonl`, and a generated `SUMMARY.md`. It ships as a single binary. This repo
**self-hosts** its own issues under `./issues/`, so the engine you build here is the engine that
tracks the work on it.

## Working on the engine

The engine is **`src/`** — one package at the repo root, no workspace. Build it with
`cargo build --release`; that binary is what every harness in this repo points at.

- **No dependencies, ever.** The binary is a single artifact a repository depends on for years,
  and every dependency is a future reason it stops building. The standard library is the whole
  toolbox.
- Lints deny `unsafe`, `unwrap`, `expect` and `panic`: a malformed tracker must produce a
  diagnostic, never a stack trace. `println!` once slipped underneath that — it unwraps its
  write, so a closed pipe panicked — which is why all output goes through one function in
  `cli.rs` that writes, flushes, and treats a gone reader as the success it is.
- `cargo fmt --all --check`, `cargo clippy --all-targets --all-features -- -D warnings`, and
  `cargo test --all` all gate CI.

**Three suites, all run in CI:**

- `cargo test --all` — the engine's own tests, including `tests/app_js.rs`, which
  lifts pure functions out of the compiled-in `assets/app.js` and runs them under `node`
  (skipped when node is absent). That asset is a string to the compiler, so nothing else would
  catch a syntax error in it. And `broken_pipe.rs`, which closes a reader on a running verb.
- `python3 conformance/run.py` — **the executable specification**, and the one to understand
  first. It **execs** the binary (`--bin`, `$TRCK_BIN`, default `target/release/trck`) and never
  imports anything, so it describes behaviour rather than implementation. A fixture is a
  starting tracker, one command, and what that command should print. **Anything a user or a
  downstream tool would notice belongs there**; internals stay in unit tests. `--min-pass` is a
  ratchet that only moves up.
- `python3 -m unittest discover -s scripts/tests` — the helper scripts under `scripts/`: the
  installer, a timestamp backfill, an id converter, the CI path classifier. None is part of the
  engine, which is why they are a separate suite rather than something gating every engine change.

Add a test for every change (TDD), in whichever of the three it belongs to.

**CI skips the engine's suites on a tracker- or prose-only pull request.** `scripts/ci_changed.py`
classifies the diff and the jobs carry `if: needs.changes.outputs.code == 'true'`; `trck check`
runs either way. It is an **allowlist** — only `issues/`, `docs/` and repository-root markdown are
skippable, because markdown elsewhere is a compiled-in asset, a conformance fixture, or the example
tracker. Uncertainty resolves to running everything: a rule that wrongly says "skippable" does not
turn a check red, it makes the checks green by never running them. So widening it means adding a
case to `scripts/tests/test_ci_changed.py` first. It cannot be `paths-ignore`, either — merging is
gated on named checks and a path-filtered workflow never reports them, leaving the pull request
waiting forever; a job skipped by an `if:` reports as skipped, which counts as passing. **The
matrix job is the exception:** skipped whole, `rust` reports under that bare name and the gated
`rust (ubuntu-latest)` never arrives, so it always runs — `changes` shrinks its matrix to one
platform and its steps carry the gate instead, with the build and `trck check` ungated.

**The quality ratchet.** `quality-report.json` is a committed snapshot of structural metrics —
function length, cognitive and cyclomatic complexity, argument counts, file size. CI runs
[ratchet](https://github.com/leonkacowicz/ratchet) over it twice: `check` fails if the report
no longer describes the code, and `compare` fails if any metric got worse than the baseline.

Existing debt is grandfathered and may only shrink. You cannot add a longer or more complex
function than what is already there without paying it down elsewhere in the same category —
and splitting something oversized passes, because the category total drops.

**So a change that touches `src/` needs `ratchet generate` and the regenerated report staged
with it.** The pre-commit hook says so rather than letting CI be the one to tell you. If a
threshold itself needs to move, that is its own commit: ratchet refuses a threshold edit in
the same change as a new violation.

**Enable the pre-commit guard once per clone:** `git config core.hooksPath scripts/hooks`. It
runs `trck check` before commits, preferring the binary in `target/` over an installed one — in
this repo the engine under change is the one that should answer.

The vocabulary is **fixed in code**, not configured — `backlog → in-progress → in-review → done`,
five priorities, three resolutions, all constants in `src/config.rs`. It used to come
from each tracker's `trck.json`; that is gone, and `check` warns about leftover keys. `trck.json`
now holds only the format version and the update channel.

## Tracking work (dogfooding)

- Use the built binary (`./target/release/trck`) for all bookkeeping; hand-edit only an
  issue's markdown **body** (Summary / Acceptance criteria / Notes). Never hand-edit `index.jsonl` or `SUMMARY.md`, and never move or
  rename issue files by hand — the verbs do that.
- Issue bodies all live in `issues/items/` — status is **not** encoded in the path; it lives only
  in `index.jsonl`. A `start`/`done` touches the index and `SUMMARY.md`, never the body file.
  (`trck repo migrate-layout` converts a pre-0.23 tracker; every verb refuses one until it runs.)
- `trck check` must pass before committing. `SUMMARY.md` is generated.
- Keep issue-tracker commits separate from engine-code commits where reasonable.
- **Nothing is vendored.** `trck` is installed on the machine or built here, never committed
  into the repository it serves — so there is no second engine that can drift from the data.

## Releasing

Bump `version` in the workspace **`Cargo.toml`** → **open a PR** → merge it once CI is green →
tag `vX.Y.Z` **on the merged commit**. `.github/workflows/release.yml` takes it from there: it
cross-builds six targets, **installs the musl artifact and runs the conformance suite against
it**, and only then creates the release and uploads the assets. A build that cannot pass its own
spec never becomes a download.

**A release commit goes through a PR like any other.** `main` is protected and its required
checks are the point — pushing a bump straight to it means the commit every published binary is
built from is the one commit nobody ran CI on. Admin permissions make the bypass possible; that
is not a reason to use it. The release workflow's own `verify` job is a stricter gate than the PR
checks, but it runs *after* the tag, so a bump that breaks the build costs a tag you then have to
delete and re-cut.

**Tag after the merge, never the branch.** A squash merge writes a *new* commit, so the branch
SHA the bump was authored on is not the SHA that lands on `main` — tagging it would point the
release at a commit that is not in the history. Merge, `git pull`, then tag `HEAD`.

The workflow fires on **any** `v*` tag, so its first job builds only when the tag equals `v` +
the workspace `Cargo.toml` version — which is what makes "tag the merged commit" load-bearing
rather than tidiness, since a tag cut anywhere else would publish binaries labelled with someone
else's version. It **skips** on a mismatch rather than failing, so a tag that is not a release of
this binary leaves no red run behind; the run summary says which happened. The check is before
the matrix, so a wrong tag costs a checkout rather than six cross-builds. **If a release you
expected never appears, read the guard's summary first.**

Targets are `x86_64-unknown-linux-{gnu,musl}`, `aarch64-unknown-linux-musl`,
`{x86_64,aarch64}-apple-darwin`, `x86_64-pc-windows-msvc`. musl is the default the installer
picks on Linux: statically linked, so it does not care whether the machine's glibc is older than
the builder's.

Install paths: `scripts/install.sh` (`curl … | sh`, computes the target from `uname`, verifies
the published `.sha256`), and `packaging/homebrew/trck.rb` for a tap. Bump the formula's
`version` in the same commit as `Cargo.toml`.

The binary has **no self-update**: whatever installed it owns the file, and a self-updater
fighting a package manager is worse than none. `trck update` answers with the upgrade path
instead of doing anything.

## Working method

- Decompose tasks into sub-tasks as much as it makes sense. Keep splitting until each
  sub-task is small and cohesive enough to be done "in one go" — once breaking it down
  further no longer makes sense, stop.
