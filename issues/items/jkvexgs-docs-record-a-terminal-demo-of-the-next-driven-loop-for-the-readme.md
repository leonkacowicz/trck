# docs: record a terminal demo of the next-driven loop for the README

## Summary

The README opens with a static SVG of `ready`. That shows what the output *looks like*, but
not what the tool actually does: derive an order from the graph and keep re-deriving it as work
closes. A short terminal recording is the only artifact that shows the loop — close a task,
ask again, get a different answer because something became unblocked.

This is the asset every promotion venue points at (README hero, Show HN post, social preview),
so it should be **generated and re-runnable**, not a hand-typed one-off: `ready`/`next` output
changes often, and a stale demo is worse than none.

## Acceptance criteria
- [ ] A recording tool is chosen and its input script is committed (see the decision in Notes).
- [ ] The demo runs against `examples/action-game` — no ad-hoc fixture tracker.
- [ ] Regenerating is a single documented command, in the same spirit as
      `python3 docs/gen-screenshots.py`, and is noted in README's Develop section.
- [ ] Output committed under `docs/img/` and embedded at the top of README.md.
- [ ] Total runtime under ~40s; readable at README width without zooming.

## Notes

### Storyboard

1. `trck deps` — the DAG. Establishes there is a real graph, not a flat checklist.
2. `trck next` — one answer, carrying the `↑urgent(#…)` marker that explains why it outranks
   its own declared priority. This is the frame that makes the point.
3. `trck done <id>` → `trck next` again — a different task, newly unblocked. The
   re-derivation is the whole idea.
4. The unattended loop: repeatedly take `next`, do the work, close it, repeat — a backlog
   walked in dependency order with no planner deciding anything.

Frames 1-3 can be recorded today. Frame 4 needs machine-readable `ready`/`next` output, which
is not yet covered by the `--json` epic #r9zefup (it has `list`/`show`/`deps` children, no
`ready`/`next`). Either add that child and depend on it, or ship frames 1-3 first and extend
the tape later.

### Open decision — which recorder

None of these are currently installed; picking one is part of this issue.

- **VHS** (charmbracelet) — a `.tape` script of keystrokes and timings renders GIF/MP4/WebM
  deterministically. Re-runnable and diffable, which matches how `docs/img/` is already
  produced. Cost: a Go binary as a dev-time dependency.
- **asciinema + agg** — records a real session; `agg` converts the cast to a GIF. Most
  authentic, and asciinema.org gives a player with selectable text. Cost: re-recording is
  manual typing every time output shifts.
- **Extend `docs/gen-screenshots.py` to emit animated SVG** — standard-library only, no new
  tooling, and animated SVG does play inline in GitHub READMEs. Cost: building a small
  terminal recorder to avoid a dependency the artifact never ships with.

Whatever is chosen, the engine's standard-library-only constraint is unaffected — this is
documentation tooling, like `docs/gen-screenshots.py`, not part of `./trck`.
