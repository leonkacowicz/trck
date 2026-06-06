# Add normalize command to rewrite index.jsonl in canonical slim form

## Summary
After #020, `index.jsonl` is written in a slimmer canonical form (default-valued fields
stripped), but slimming only happens as a side effect of a *write* — `save_index` is
reached only through `finalize`, i.e. via a mutating verb (`new`/`mv`/`set`/`dep`/`rename`).
`check` and `summary` are read-only. So an index produced by an older engine (e.g. a
consumer repo that just ran `trck update`) stays verbose until the next real mutation, and
there is no deterministic, no-op way to tidy it on demand.

Add a `trck normalize` verb that re-serializes the index in canonical form (and regenerates
`SUMMARY.md`) without changing any issue's data.

## Acceptance criteria
- [x] `trck normalize` loads the index and runs it back through the standard finalize pass
      (`save_index` + `write_summary` + `validate`) — no field values change, only the
      on-disk serialization is canonicalized (default-valued trck-owned fields stripped,
      CANON order applied, custom keys preserved).
- [x] Running it on an already-canonical index is a no-op: `index.jsonl` is byte-identical
      before and after (idempotent).
- [x] Running it on a verbose/old-format index (full of `milestone: null`, `depends_on: []`,
      etc.) rewrites every row into the slim form.
- [x] It prints a short confirmation (e.g. `normalized <path> (N issues)`), consistent with
      `cmd_summary`'s output style.
- [x] Wired into argparse as a subcommand with a help string; appears in `trck --help`.
- [x] Does not move or rename any issue files and does not touch issue markdown bodies —
      index + SUMMARY only.

## Tests (TDD)
- [x] `normalize` on a verbose index strips default-valued fields from every row.
- [x] `normalize` is idempotent (second run is byte-identical to the first).
- [x] `normalize` preserves non-default and custom/unknown field values.
- [x] `normalize` regenerates `SUMMARY.md`.

## Notes
- Implementation is tiny: `cmd_normalize` = `ctx = build_ctx(args)` →
  `finalize(ctx, load_index(ctx))` → print. `finalize` already bundles save + summary +
  validate, so normalize inherits the consistency check for free.
- Naming: `normalize` over `fmt`/`tidy` — describes intent (canonical form) without
  implying it reflows prose. Considered folding into `check --fix`, but a read-only `check`
  that silently writes is surprising; a dedicated verb is clearer and composes with the
  pre-commit hook.
- Touchpoints: new `cmd_normalize` near `cmd_summary` (`trck:888`), and a `sub.add_parser`
  entry in the argparse block (near `trck:1134`). Update the verbs table in
  `issues/CLAUDE.md` and the `--help` epilog if present.
