# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

`trck` is a single-file, standard-library-only, in-repo issue tracker. The entire engine is the
executable file **`./trck`** at the repo root (no extension; it is also importable as a module
for tests). This repo **self-hosts** its own issues under `./issues/` — drive them with `./trck`
from the repo root (discovery finds `issues/` via its `trck.json`).

## Working on the engine

- The whole engine is `./trck`. Keep it **standard-library only** — no third-party imports, ever.
- Run the full suite: `python3 -m unittest discover -s tests -v`. Add a test for every change (TDD).
  Run one module: `python3 -m unittest tests.test_paths -v`; one case: `python3 -m unittest tests.test_paths.TestClass.test_method`.
- `tests/helpers.py::load_trck()` imports the extensionless file via `importlib`
  (`SourceFileLoader`, required on Python 3.12+/3.14).
- **Never let a test overwrite the real `./trck`.** Anything that writes to the engine file —
  `update` and `init` — does so through the module global `SELF_PATH`. Tests for those verbs
  reassign `mod.SELF_PATH` to a throwaway temp copy first. Follow that pattern.
- The file is organized in bands: constants → config/discovery → index I/O → scan/validate →
  SUMMARY/finalize → networking seam → command handlers → argparse/`main`.
- The vocabulary is **data-driven, not hard-coded**: statuses (with `initial`/`terminal` roles),
  verb aliases (`start`, `done`), priorities, kinds, and resolutions all come from each tracker's
  `trck.json` (see `issues/trck.json`). Code reads them via the `load_config`/`status_*`/`check_*`
  helpers — don't bake status or priority names into the engine.

## Tracking work (dogfooding)

- Use `./trck` for all bookkeeping; hand-edit only an issue's markdown **body** (Summary /
  Acceptance criteria / Notes). Never hand-edit `index.jsonl` or `SUMMARY.md`, and never move or
  rename issue files by hand — the verbs do that.
- `./trck check` must pass before committing. `SUMMARY.md` is generated.
- Keep issue-tracker commits separate from engine-code commits where reasonable.
- **This canonical repo keeps no vendored engine copy**: `./trck` (root) runs directly against
  `./issues/`. (`trck init` vendors `issues/trck` for *consumer* repos; this repo was set up with
  `trck init --no-vendor` so there's no second engine to drift.)

## Releasing

Bump `__version__` in `trck` → commit → tag `vX.Y.Z` → create a GitHub Release. `trck update`
consumes the latest release on the stable channel.

## Working method

- Decompose tasks into sub-tasks as much as it makes sense. Keep splitting until each
  sub-task is small and cohesive enough to be done "in one go" — once breaking it down
  further no longer makes sense, stop.
