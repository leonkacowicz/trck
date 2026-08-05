# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`trck` is a single-file, standard-library-only, in-repo issue tracker. It **ships** as one
executable file — **`./trck`** at the repo root (no extension; also importable as a module for
tests) — but that file is **generated**: the source of truth is the **`src/trck/`** package,
flattened into `./trck` by **`build.py`**. This repo **self-hosts** its own issues under
`./issues/` — drive them with `./trck` from the repo root (discovery finds `issues/` via its
`trck.json`).

## Working on the engine

- **Edit `src/trck/*.py`, never `./trck`.** `./trck` is a build artifact (like `SUMMARY.md`) —
  hand-edits are overwritten and rejected by the pre-commit hook. Keep the engine **standard-library
  only** — no third-party imports, ever.
- **Build:** `python3 build.py` regenerates `./trck`; `python3 build.py --check` fails if `./trck`
  is out of sync with `src/`. The build is a **byte-exact amalgamation**: it emits
  `src/trck/__init__.py` (the header — shebang, license, docstring, `__future__`, the one stdlib
  import block) verbatim, then each module with its top-level imports stripped (an `ast`-based strip,
  so `import`/`from` lines inside template strings are never touched).
- The package maps ~1:1 onto the engine's original bands, in build order (`build.py::MANIFEST`):
  `constants → config → index → graph → scan → render → summary → finalize → net → templates →
  cmd_mutate → cmd_query → cmd_maint → cmd_selfmgmt → cli`. Module-level constants run at import
  time, so **order matters** — a name must be defined before a later module's top-level code uses it.
- **`src/trck/` is build input, not a runnable package.** It has intentional import cycles and is
  never imported/executed; modules carry sibling/stdlib imports **only so editors (pyright/Pylance/
  PyCharm) resolve symbols**, and the build strips them all. Add the matching `from .mod import name`
  when you reference a new sibling symbol — it keeps the editor clean and is harmless to the build.
- **Enable the pre-commit guard once per clone:** `git config core.hooksPath scripts/hooks`. It runs
  `build.py --check` (engine in sync with src) and `trck check` (tracker consistent) before commits.
- **Three suites**, all run in CI:
  - `python3 -m unittest discover -s tests -v` — the engine. `tests/__init__.py` **rebuilds
    `./trck` from `src/` first**, so it always reflects your source edits; commit the regenerated
    `./trck` alongside the change. One module: `python3 -m unittest tests.test_paths -v`; one
    case: `…tests.test_paths.TestClass.test_method`.
  - `python3 -m unittest discover -s scripts/tests -v` — standalone one-shots under `scripts/`
    that don't import the engine. Separate because tests shelling out to `git` for a migration
    nobody will run again shouldn't gate every engine change.
    - `cargo test --all` — the Rust engine, including `crates/trck/tests/app_js.rs`, which lifts
    pure functions out of the compiled-in `assets/app.js` and runs them under `node` (skipped
    when node is absent). That asset is a string to the compiler, so nothing else would catch a
    syntax error in it.
  - `python3 conformance/run.py` — the executable spec (`conformance/README.md`). It **execs**
    the binary (`TRCK_BIN`, default `./trck`) and never imports it, so it will run unchanged
    against the Rust engine. Anything a user or downstream tool would notice belongs there;
    internals stay in `tests/`.

  Add a test for every change (TDD), in whichever of the three it belongs to.
- `tests/helpers.py::load_trck()` imports the generated `./trck` via `importlib`
  (`SourceFileLoader`, required on Python 3.12+/3.14).
- **Tests that write to the engine file** — `update`/`init` — go through the module global
  `SELF_PATH`; those tests reassign `mod.SELF_PATH` to a throwaway temp copy first. Follow that
  pattern. (This is separate from the intentional build-before-test rebuild of `./trck`.)
- The vocabulary is **fixed in code**, not configured — `backlog → ongoing → in-review → done`,
  five priorities, three resolutions, all constants in `src/trck/config.py`. It used to come from
  each tracker's `trck.json`; that is gone, and `check` warns about leftover keys. Read it through
  the `status_*`/`is_*`/`check_*` helpers, which still take a `cfg` they ignore (every call site
  threads one). `trck.json` now holds only the format version and the update channel.

## Tracking work (dogfooding)

- Use `./trck` for all bookkeeping; hand-edit only an issue's markdown **body** (Summary /
  Acceptance criteria / Notes). Never hand-edit `index.jsonl` or `SUMMARY.md`, and never move or
  rename issue files by hand — the verbs do that.
- Issue bodies all live in `issues/items/` — status is **not** encoded in the path; it lives only
  in `index.jsonl`. A `start`/`done` touches the index and `SUMMARY.md`, never the body file.
  (`trck repo migrate-layout` converts a pre-0.23 tracker; every verb refuses one until it runs.)
- `./trck check` must pass before committing. `SUMMARY.md` is generated.
- Keep issue-tracker commits separate from engine-code commits where reasonable.
- **This canonical repo keeps no vendored engine copy**: `./trck` (root) runs directly against
  `./issues/`. (`trck init` vendors `issues/trck` for *consumer* repos; this repo was set up with
  `trck init --no-vendor` so there's no second engine to drift.)

## The Rust port (`#sp2rwzx`)

`crates/trck/` is the Rust engine, and it is **empty on purpose**. The conformance suite
(`conformance/`) runs against a *binary*, so the port is measured from its first commit rather
than assessed at the end by reading code: CI runs the fixtures against `target/release/trck` with
`--min-pass 0` and reports the pass rate. That floor is a ratchet — raise it as fixtures go green,
and the build fails if the number ever drops.

- `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`, `cargo test --all`.
- **No dependencies**, same constraint as the Python engine: the binary is a single artifact a
  repo depends on for years. Lints deny `unsafe`, `unwrap`, `expect` and `panic` — a bad tracker
  must produce a diagnostic, not a stack trace.
- `python3 conformance/run.py --compare-bin target/release/trck` is the oracle: run both engines
  over every fixture and diff them against each other, catching disagreements nobody wrote a
  golden for.

## Releasing

Bump `__version__` in **`src/trck/constants.py`** → `python3 build.py` (regenerate `./trck`) →
`python3 build.py --check` → commit `./trck` together with the source → tag `vX.Y.Z` → create a
GitHub Release. `trck update` consumes the latest release on the stable channel.

## Working method

- Decompose tasks into sub-tasks as much as it makes sense. Keep splitting until each
  sub-task is small and cohesive enough to be done "in one go" — once breaking it down
  further no longer makes sense, stop.
