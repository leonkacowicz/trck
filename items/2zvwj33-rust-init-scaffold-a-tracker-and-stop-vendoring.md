# rust: init — scaffold a tracker, and stop vendoring

## Summary
`init` is the one verb the Rust engine still refuses: `trck init` answers `` `init` is not
implemented yet in the Rust engine ``. Every other verb is done, so this is what stands between
the port and the cutover — retire the Python engine today and there is no way left to create a
tracker at all, which is a worse regression than anything the port has fixed.

The Python verb is small (`cmd_init`): resolve the target dir, refuse an existing tracker without
`--force`, write `trck.json`, copy in the `CLAUDE.md` and `README.md` scaffolds, optionally
install the pre-commit hook. The Rust engine already has `install-hook` and the config writer;
what it lacks is the two doc templates, which live in `src/trck/templates.py` today and would
become compiled-in assets beside `app.js`.

The one deliberate difference: **no vendoring**. Vendoring existed because the engine was a
Python file that had to match the data's format version, and a committed copy was the only pin.
A binary is not copyable across platforms, and the format guard (`#9fajv3x`) now does the job
vendoring was standing in for. So the Rust `init` writes no engine, and `--no-vendor` becomes
the only behaviour rather than a flag.

## Acceptance criteria
- [x] `trck init [dir] [--force] [--hook]` scaffolds `trck.json`, `CLAUDE.md` and `README.md`.
- [x] No engine is copied into the tracker. `--no-vendor` is **accepted and does nothing**: it
      asks for the only behaviour there is now, so refusing it would mean erroring on a request
      already satisfied, and every script that learned to pass it keeps working.
- [ ] ~~The vendored-engine branch of discovery (`discovery.rs`) goes with it.~~ **Deferred to
      the cutover (`#djx63gk`).** Removing it is a divergence from the Python engine, which
      still resolves a tracker that way, and the differential oracle would start reporting it
      as a disagreement while both engines are alive. It costs nothing to carry until the swap.
- [x] Conformance fixtures cover an init over an existing tracker and `--force`. **Not a fresh
      init**: the runner creates the tracker before running `cmd`, so no fixture can reach a
      directory that is not already one. Fresh scaffolding is covered by unit tests in
      `init.rs` instead, and the gap is deliberate rather than overlooked.
- [x] The scaffolded `CLAUDE.md` matches what the Python engine wrote, minus the vendoring
      sentences — the two engines' scaffolds should not disagree on the day of the swap.
      Verified byte-for-byte: `trck.json` and `CLAUDE.md` identical, `README.md` differing by
      exactly the sentence about running a vendored `./trck`.
