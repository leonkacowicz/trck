# trck examples

Demo trackers you can poke at to see what `trck` does — without touching this repo's
own `./issues`. Each example is a self-contained tracker (its own `trck.json`); point
`trck` at it with `--dir`.

## `action-game/` — a fictional 2D action-platformer

A hand-built tracker for an imaginary indie game, designed to exercise every structural
feature: a multi-level epic tree, a real dependency DAG, the full status/kind/priority
vocabulary, cross-cutting labels, and all three resolutions on closed work. 35 issues.

Run everything against it with `--dir examples/action-game` (from the repo root):

```bash
./trck --dir examples/action-game tree        # the whole forest, nested
./trck --dir examples/action-game ready        # what you could pick up right now
./trck --dir examples/action-game next         # the single best next task
./trck --dir examples/action-game deps --graph # the dependency DAG, lazygit-style
./trck --dir examples/action-game show 21      # one issue's metadata + prose
```

> Tip: export `TRCK_DIR=examples/action-game` to drop the `--dir` flag for a session.

### What to look at

| Feature | Where to see it |
|---|---|
| **3-level hierarchy** | `tree` — `#001 Player movement & combat` → nested epic `#002 Combat system` → leaf tasks. |
| **Points roll up to epics** | `summary` (or `tree`) — each epic shows `% (done/total pts)`; only leaves carry points. |
| **Dependency DAG** | `deps --graph` — fan-out from `#016 Sprite atlas tool` (blocks 4 art tasks) and fan-in at `#021 Level 1` (needs movement + art from different epics). |
| **`ready` vs `next`** | `ready` hides anything still blocked by an unfinished dependency; `next` is the top pick. |
| **`deps` for one issue** | `deps 21` — what it requires and what it blocks. |
| **All 5 kinds** | task, epic, bug, story, investigation — e.g. `list --kind bug`. |
| **Priorities (soft order)** | `list --status '!done' --sort priority`. |
| **Labels (the cross-cutting axis)** | `list --label combat` — labels categorize *across* the hierarchy. |
| **Statuses** | `list --status ongoing`; folders `backlog/ ongoing/ done/` mirror them. |
| **Resolutions on closed work** | `show 33` (superseded), `show 34` (duplicate), `show 35` (wontfix). |

### How it was built

Everything was generated through the engine — `trck new / set / dep / label / start /
done` — so `index.jsonl` and `SUMMARY.md` are engine-written, never hand-edited. Only the
markdown **bodies** (Summary / Acceptance criteria / Notes) are hand-authored, which is
exactly the intended workflow. `./trck --dir examples/action-game check` passes clean.

Feel free to mutate it — start an issue, add a dependency, close something — and watch
`SUMMARY.md` and the graphs change. It's a sandbox; nothing here affects the real tracker.
