# CLAUDE.md — trck

`trck` is a single-file, standard-library-only, in-repo issue tracker. The entire engine is the
executable file **`./trck`** at the repo root (no extension; it is also importable as a module
for tests). This repo **self-hosts** its own issues under `./issues/` — drive them with `./trck`
from the repo root (discovery finds `issues/` via its `trck.json`).

## Working on the engine

- The whole engine is `./trck`. Keep it **standard-library only** — no third-party imports, ever.
- Run the tests: `python3 -m unittest discover -s tests -v`. Add a test for every change (TDD).
- `tests/helpers.py::load_trck()` imports the extensionless file via `importlib`
  (`SourceFileLoader`, required on Python 3.12+/3.14).
- **Never let a test overwrite the real `./trck`.** Anything that writes to the engine file —
  `update` and `init` — does so through the module global `SELF_PATH`. Tests for those verbs
  reassign `mod.SELF_PATH` to a throwaway temp copy first. Follow that pattern.
- The file is organized in bands: constants → config/discovery → index I/O → scan/validate →
  SUMMARY/finalize → networking seam → command handlers → argparse/`main`.

## Tracking work (dogfooding)

- Use `./trck` for all bookkeeping; hand-edit only an issue's markdown **body** (Summary /
  Acceptance criteria / Notes). Never hand-edit `index.jsonl` or `SUMMARY.md`, and never move or
  rename issue files by hand — the verbs do that.
- `./trck check` must pass before committing. `SUMMARY.md` is generated.
- Keep issue-tracker commits separate from engine-code commits where reasonable.
- **This canonical repo keeps no vendored engine copy**: `./trck` (root) runs directly against
  `./issues/`. (`trck init` vendors `issues/trck` for *consumer* repos; here that copy is
  intentionally absent to avoid two engines drifting — see issue about `init --no-vendor`.)

## Releasing

Bump `__version__` in `trck` → commit → tag `vX.Y.Z` → create a GitHub Release. `trck update`
consumes the latest release on the stable channel.
