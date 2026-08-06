# cutover: un-vendor, retire the Python engine, rewrite the docs

## Summary
The last step, and the irreversible one. Only worth taking when the conformance suite passes
fully against the Rust binary and the differential runner finds no divergence on this repo's own
tracker.

## Acceptance criteria
- [ ] `trck init` stops vendoring; the vendored-engine resolution path goes.
- [ ] `src/trck/`, `build.py`, the amalgamation and its MANIFEST discipline all removed.
- [ ] The pre-commit hook, `.gitattributes` merge drivers and CI all point at the installed
      binary.
- [ ] **`trck check` survives the deletion.** It runs today as a step in the Python `test:`
      job of `.github/workflows/ci.yml` — the job this removes wholesale — so it leaves with
      it unless deliberately carried over. Losing it would be silent: CI stays green, and the
      repo simply stops noticing an inconsistent tracker. See `#k6g7kvf`, which is the same
      check for everyone else.
- [ ] The 706 tests under `tests/` go with `src/trck/`. Record what that leaves behind — 175
      Rust tests and 227 conformance fixtures — and name anything the Python suite covered
      that neither does, rather than discovering it later.
- [ ] README, `CLAUDE.md`, `issues/CLAUDE.md` and the skill rewritten — several of them describe
      the single-file amalgamation as the central fact about the project.
- [ ] A documented answer for a contributor who does not have trck installed, since that is
      exactly what vendoring used to solve.
- [ ] A final release of the Python engine, tagged and noted as the last of its line.
