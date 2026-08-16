# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`trck` is an in-repo issue tracker: one markdown file per issue, all metadata in `index.jsonl`,
and a generated `SUMMARY.md`. It ships as a single binary. Those files can be a directory in the
working tree or the root of a git ref; this repo **self-hosts** its own issues on the
`trck-issues` branch, so the engine you build here is the engine that tracks the work on it —
including the work of moving them there.

## Working on the engine

The engine is **`src/`** — one package at the repo root, no workspace. Build it with
`cargo build --release`; that binary is what every harness in this repo points at.

- **One self-contained binary is the invariant — not an empty `[dependencies]`.** What a
  repository depends on for years is a single artifact with nothing to install beside it, so a
  crate is fine as long as it links statically in. What is ruled out is anything that has to be
  *present on the user's machine* at run time: a shared library, a language runtime, a package
  tree. The one exception is **`git`**, which is definitional rather than a choice — the tracker
  lives in a git repository and the engine drives git plumbing directly (`src/git/`). Weigh a
  crate on build cost and supply-chain surface like any other project; std is still the default,
  it is just no longer the whole toolbox.
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
  installer, a timestamp backfill, an id converter, the CI path classifier, the pre-commit hook.
  None is part of the engine, which is why they are a separate suite rather than something gating
  every engine change.

Add a test for every change (TDD), in whichever of the three it belongs to.

**CI skips the engine's suites on a prose-only pull request.** `scripts/ci_changed.py` classifies
the diff and the jobs carry `if: needs.changes.outputs.code == 'true'`. It is an **allowlist** —
only `docs/` and repository-root markdown are skippable, because markdown elsewhere is a
compiled-in asset, a conformance fixture, or the example tracker. Uncertainty resolves to running
everything: a rule that wrongly says "skippable" does not turn a check red, it makes the checks
green by never running them. So widening it means adding a case to
`scripts/tests/test_ci_changed.py` first. It cannot be `paths-ignore`, either — merging is gated on
named checks and a path-filtered workflow never reports them, leaving the pull request waiting
forever; a job skipped by an `if:` reports as skipped, which counts as passing. **The matrix job is
the exception:** skipped whole, `rust` reports under that bare name and the gated
`rust (ubuntu-latest)` never arrives, so it always runs — `changes` shrinks its matrix to one
platform and its steps carry the gate instead, with the build ungated.

`issues/` used to head that allowlist. The tracker is on the `trck-issues` branch now, which a pull
request against `main` cannot reach, so there is no tracker-only pull request to exempt — and a diff
that does touch `issues/` means someone put that directory back, which is a change to build.
**`trck check` moved out of `ci.yml` entirely**, into a `tracker.yml` that lives **on the
`trck-issues` branch**, where it fires on a push and checks the pushed commit through `--ref`.
On the branch and not here, because GitHub resolves a workflow from the ref the event happened on:
kept on `main`, a `push: branches: [trck-issues]` trigger never fires at all — no error, no run,
nothing. It sat here for two merges before that was noticed, so
`scripts/tests/test_ci_changed.py` now asserts that **no** workflow on `main` names that branch.
Living on a branch `main` never merges also keeps it from becoming a required check: named in
branch protection it would never report on a pull request, which would wait for it forever.

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

**`compare`'s baseline is measured, not read.** `scripts/ratchet_baseline.sh` checks the base
revision out into a temp directory and runs `ratchet generate` over it with the same binary
that measured the head, then compares the two files. Reading the base commit's committed report
instead — `compare --base` — only works while both sides were written by the same ratchet, and
the day the tool's own metrics change every file reads as a large regression on a change that
touched nothing. **Ratchet is pinned in the workflow, and your machine's copy will drift from
it.** When `ratchet check` starts failing on a clean checkout, compare `ratchet --version`
against the pin in `.github/workflows/ci.yml` before believing the numbers.

**Enable the pre-commit guard once per clone:** `git config core.hooksPath scripts/hooks`. It
runs `ratchet check`, and nothing else. It deliberately does **not** run `trck check`: the tracker
is not in this working tree, so no commit here can make it inconsistent — the write verbs commit
to the ref through plumbing, validating as they go, and never pass through a pre-commit hook.
`scripts/tests/test_pre_commit.py` asserts the absence, because a guard that grew a tracker check
back would refuse any commit that touched a directory called `issues/`.

The vocabulary is **fixed in code**, not configured — `backlog → in-progress → in-review → done`,
five priorities, three resolutions, all constants in `src/config.rs`. It used to come
from each tracker's `trck.json`; that is gone, and `check` warns about leftover keys. `trck.json`
now holds only the format version and the update channel.

## Tracking work (dogfooding)

**The tracker is not in this working tree.** It lives at the root of the `trck-issues` branch,
and every verb finds it there by itself. Run them from anywhere in the repo, with no flags:

```
trck ready                       # what is unblocked
trck new "title" --priority high # files it, commits it, pushes it
trck start <id>                  # and so does every other write verb
```

- Use the built binary (`./target/release/trck`) for all bookkeeping. It has to be one that can
  read a ref — v0.30.0 or newer, or a build from `main`. Anything older looks for a directory,
  does not find one, and says so.
- **A write verb is the whole transaction.** It builds a commit on `trck-issues` through git
  plumbing and pushes it — no checkout, no staging, no worktree, and your branch and working tree
  are untouched however dirty they are. Nothing here needs a branch, a PR or a review: an issue
  row cannot break the build.
- **A rejected push is not a failure.** Someone else landed first, so the operation is *replayed*
  on top of theirs from its `Trck-Op:` trailer and pushed again. Nothing is ever forced.
- **A write that cannot reach the remote still succeeded.** The commit is anchored locally and the
  verb says `(N unpushed changes — run `trck sync`)`. `trck sync` pushes them when you are back.
- Hand-edit only an issue's markdown **body** (Summary / Acceptance criteria / Notes), through
  `trck edit <id>`, which opens `$VISUAL`/`$EDITOR` on the body and commits what you write. There
  is no file to open by hand — `--body`, `--body-file` and `--empty` say where the prose comes
  from when no editor is wanted. Never try to hand-edit `index.jsonl` or `SUMMARY.md`.
- Status is **not** encoded in any path; it lives in `index.jsonl` alone. `SUMMARY.md` is
  generated on every write.
- **Nothing is vendored.** `trck` is installed on the machine or built here, never committed
  into the repository it serves — so there is no second engine that can drift from the data.

The tracker branch has its own CI (`tracker.yml`, which lives on that branch — see above), so
`trck check` runs on every tracker commit without anyone remembering to.

### If you need to see the tracker as files

You cannot, and the verbs will tell you so rather than printing a path that is not there:
`trck path`, `trck which` and `list --paths` all refuse against a ref-backed tracker, rather than
printing a relative path that reads as real and is not there. Use `trck show <id>` for a body,
`git show trck-issues:items/<id>-<slug>.md` to read one raw, or check the branch out somewhere —
but if you do, **detach it**: a live checkout of `trck-issues` has its `HEAD` moved under it by
the next write, and `git status` there then shows that write inverted (#sny6t9q).

## Releasing

Bump `version` in the workspace **`Cargo.toml`** → **open a PR** → merge it once CI is green →
tag `vX.Y.Z` **on the merged commit**. `.github/workflows/release.yml` takes it from there: it
cross-builds six targets, **installs the musl artifact and runs the conformance suite against
it**, and only then creates the release and uploads the assets. A build that cannot pass its own
spec never becomes a download.

**A release commit goes through a PR like any other.** `main` is protected and its required
checks are the point — pushing a bump straight to it means the commit every published binary is
built from is the one commit nobody ran CI on. Admin permissions make the bypass possible; that
is not a reason to use it. **This rule is about code.** A tracker commit cannot affect the build —
it does not land on `main` at all, but on `trck-issues`, where its own workflow checks it. A
version bump is code, whatever else it touches.
The release workflow's own `verify` job is a stricter gate than the PR checks, but it runs
*after* the tag, so a bump that breaks the build costs a tag you then have to delete and re-cut.

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
