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
- Run the full suite: `python3 -m unittest discover -s tests -v`. `tests/__init__.py` **rebuilds
  `./trck` from `src/` first**, so the suite always reflects your source edits — commit the
  regenerated `./trck` alongside the change. Add a test for every change (TDD). Run one module:
  `python3 -m unittest tests.test_paths -v`; one case: `…tests.test_paths.TestClass.test_method`.
- `tests/helpers.py::load_trck()` imports the generated `./trck` via `importlib`
  (`SourceFileLoader`, required on Python 3.12+/3.14).
- **Tests that write to the engine file** — `update`/`init` — go through the module global
  `SELF_PATH`; those tests reassign `mod.SELF_PATH` to a throwaway temp copy first. Follow that
  pattern. (This is separate from the intentional build-before-test rebuild of `./trck`.)
- The vocabulary is **data-driven, not hard-coded**: statuses (with `initial`/`terminal` roles),
  verb aliases (`start`, `done`), priorities, kinds, and resolutions all come from each tracker's
  `trck.json` (see `issues/trck.json`). Code reads them via the `load_config`/`status_*`/`check_*`
  helpers — don't bake status or priority names into the engine.

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

## Releasing

Bump `__version__` in **`src/trck/constants.py`** → `python3 build.py` (regenerate `./trck`) →
`python3 build.py --check` → commit `./trck` together with the source → tag `vX.Y.Z` → create a
GitHub Release. `trck update` consumes the latest release on the stable channel.

## Working method

- Decompose tasks into sub-tasks as much as it makes sense. Keep splitting until each
  sub-task is small and cohesive enough to be done "in one go" — once breaking it down
  further no longer makes sense, stop.
