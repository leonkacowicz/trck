# trck examples

Demo trackers you can poke at to see what `trck` does — without touching this repo's
own `./issues`. Each example is a self-contained tracker (its own `trck.json`); point
`trck` at it with `--dir`.

## `action-game/` — a fictional 2D action-platformer

A hand-built tracker for an imaginary indie game, designed to exercise every structural
feature: a multi-level epic tree, a real dependency DAG **that crosses the hierarchy** (epics
depend on epics; children inherit their parent's blocks), the full status/kind/priority
vocabulary, cross-cutting labels, and all three resolutions on closed work. 35 issues.

Run everything against it with `--dir examples/action-game` (from the repo root):

```bash
trck --dir examples/action-game tree           # the active forest (settled subtrees hidden)
trck --dir examples/action-game tree --all     # ...including done/settled work
trck --dir examples/action-game ready          # what you could pick up right now
trck --dir examples/action-game next           # the single best next task
trck --dir examples/action-game deps           # the dependency DAG, lazygit-style
trck --dir examples/action-game show exg4e3b   # one issue's metadata + prose
trck --dir examples/action-game html --out /tmp/game.html   # ...the whole thing in a browser
```

> Tip: export `TRCK_DIR=examples/action-game` to drop the `--dir` flag for a session.

### What to look at

| Feature | Where to see it |
|---|---|
| **3-level hierarchy** | `tree` — `#j7n7grh Player movement & combat` → nested epic `#6ekkhbe Combat system` → leaf tasks. |
| **Points roll up to epics** | `summary` (or `tree`) — each epic shows `% (done/total pts)`; only leaves carry points. |
| **Dependency DAG** | `deps` — fan-out from `#adejzqs Sprite atlas build tool` (blocks 4 art tasks) and fan-in at `#exg4e3b Level 1` (needs movement + art from different epics). |
| **Deps climb the hierarchy** | `tree qw3rnfb` — the *World building* epic `needs #6ekkhbe`, so **every** level task inherits the block. `ready qw3rnfb` is empty: nothing in the subtree is actionable until the *Combat system* epic is done. |
| **Depend on a whole subtree** | `deps qw3rnfb` — a single epic→epic edge (*World building* → *Combat system*) waits on **all** of combat; `Combat SFX` (in the Audio epic) likewise depends on the whole *Combat system* epic, not one task. |
| **Put the edge on the parent** | *Combat system* depends on `#5qmdpg6 Walk / run / jump` once, at the epic; its six children inherit it and store **no** edge of their own (`show 2sg3uhd` — Melee — has an empty `depends_on`). |
| **`ready` vs `next`** | `ready` hides anything still blocked by an unfinished dependency (direct **or inherited**); `next` is the top pick — here `next` is a combat task, since combat gates the most downstream work. |
| **Subtree-scoped `ready`/`next`** | `ready 6ekkhbe` / `next 6ekkhbe` — actionable leaves within just the *Combat system* subtree; contrast `ready qw3rnfb` (empty, all inherited-blocked). |
| **`deps` for one issue** | `deps exg4e3b` — its directed line; `deps exg4e3b --requires` / `--blocks` for just one cone. |
| **All 5 kinds** | task, epic, bug, story, investigation — `kind` is a custom field, so `list --field kind=bug`. |
| **Priorities (soft order)** | `list --status '!done' --sort priority`. |
| **Labels (the cross-cutting axis)** | `list --label combat` — labels categorize *across* the hierarchy. |
| **Statuses** | `list --status in-progress`; status is an index field, so every body stays in `items/`. |
| **Resolutions on closed work** | `show nnxgpen` (superseded), `show 2xen4un` (duplicate), `show ypu9mn9` (wontfix) — settled standalone tasks, so hidden from the default `tree` (`tree --all` to see them). |
| **Default view hides settled work** | `tree` vs `tree --all` — done items under open epics (`#5qmdpg6`, `#adejzqs`) stay as context; the three settled standalone tasks drop off. |

### Parent × dependency — the subtle, high-value part

Dependencies **climb the parent hierarchy**, so one well-placed edge covers a whole subtree on
both ends. Three edges in this tracker show the behaviors that flat leaf-to-leaf edges can't:

1. **A prerequisite shared by a whole epic → put it on the epic.**
   *Combat system* depends on `#5qmdpg6 Walk / run / jump controller`. All six combat children
   inherit that block; none restate it. (`deps` and `ready 6ekkhbe` honour it; `show 2sg3uhd`
   shows an empty `depends_on` on the child itself.)

2. **Needing another epic wholesale → depend on the parent.**
   *World building & levels* depends on the entire *Combat system* epic (`deps qw3rnfb`): you
   can't lock down level layouts until enemies, hitboxes, and damage exist. Because both ends are
   epics, the edge means "every world task waits for every combat task" — which is why
   `ready qw3rnfb` is empty while combat is unfinished, and `next` steers you into combat first.

3. **A leaf that needs a whole subtree.**
   `Combat SFX` (under the *Audio* epic) depends on the whole *Combat system* epic — the SFX pass
   comes after combat mechanics are final, not after any single combat task.

Try it: run `ready` (global), then `start`/`done` your way through the *Combat system* leaves and
watch the World-building subtree light up in `ready` as its inherited block clears.

### How it was built

Everything was generated through the engine — `trck new / set / dep / label / start /
done` — so `index.jsonl` and `SUMMARY.md` are engine-written, never hand-edited. Only the
markdown **bodies** (Summary / Acceptance criteria / Notes) are hand-authored, which is
exactly the intended workflow. `trck --dir examples/action-game check` passes clean.

Feel free to mutate it — start an issue, add a dependency, close something — and watch
`SUMMARY.md` and the graphs change. It's a sandbox; nothing here affects the real tracker.
