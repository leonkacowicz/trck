# Clean up minor lint and cruft in the engine

## Summary
A handful of small, low-risk style/cruft items found during a hygiene pass. None change
behavior; grouped into one issue since each is a one- or two-line fix.

## Acceptance criteria
- [ ] **Dead defensiveness** — `cmd_new` (`trck:1003`):
      `points = DEFAULT_POINTS if rawpoints is None else int(str(rawpoints))`.
      `--points` is already `type=int` in argparse, so `int(str(rawpoints))` is
      redundant; simplify to use the int directly.
- [ ] **Empty f-string** — `cmd_mv` (`trck:1038`):
      `die(f"--resolution is only valid when moving to a terminal status")` has no
      placeholders (flake8 F541); drop the `f` prefix.
- [ ] **Semicolon-joined statements** — `leaf_rollup` (`trck:779`):
      `pdone += a; ptotal += b; ndone += c; ntotal += d` is the one spot breaking the
      file's one-statement-per-line style; split onto separate lines.
- [ ] **Stray blank line** — `build_parser` (`trck:1624-1625`): double blank line
      between the `label` and `show` subparser blocks; collapse to one.
- [ ] Test suite still passes (these are non-functional).

## Notes
- Double-reads (`cmd_check`/`finalize`) were intentionally split out into a separate
  issue and are **not** in scope here.
- Type-hint inconsistency (a few bare-signature functions) was noted in the hygiene
  pass but left out of scope; file separately if desired.
