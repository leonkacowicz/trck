# trck

A deterministic, single-file, **standard-library-only** issue tracker that lives *inside*
your repo. Status is the folder a markdown file sits in; all other metadata lives in
`index.jsonl`; `SUMMARY.md` is generated; only issue *bodies* are hand-authored — so the
tracker can't drift. `trck` is the generalized successor to the original `track` script.

- **One file, zero dependencies.** Just Python 3. Vendor it into a repo and commit it, or
  install it once on your `PATH`.
- **Git-friendly & agent-friendly.** Plain text, line-oriented `index.jsonl`, generated
  `SUMMARY.md`, and a hand-edited markdown body per issue.
- **Vocabulary-agnostic.** Statuses, priorities, kinds, resolutions, and verb aliases are
  configurable per repo; sensible defaults work with zero config.

## Install (global)

```bash
curl -fsSL https://raw.githubusercontent.com/leonkacowicz/trck/main/trck \
  -o ~/.local/bin/trck && chmod +x ~/.local/bin/trck
```

Then, in any repo:

```bash
trck init                       # scaffold ./issues (config + a vendored copy of trck)
                                # `trck init <dir>` for a custom dir; `--no-vendor` skips the engine copy
trck new "Fix login bug" --priority high
trck start 1                    # move to the configured 'start' status (default: ongoing)
trck done 1 --resolution wontfix
trck list
trck tree
```

`trck` finds its tracker by walking up from your current directory to the folder containing
`trck.json`, so it works from anywhere in the repo. Override with `--dir PATH` or `$TRCK_DIR`.

## Vendored / CI use

`trck init` drops a committed copy at `issues/trck`. CI and auditing use that copy with no
global install:

```bash
./issues/trck check          # nonzero exit if the tracker is inconsistent
```

## Self-update

```bash
trck update            # pull the latest stable release and atomically replace the running file
trck update --check    # report what's available, write nothing
trck update --ref v0.3.0   # update to a specific tag/branch
```

The download is validated (`compile()` + a sanity check) before the file is atomically
replaced; a failed update leaves your current `trck` untouched. Commit the resulting change to
the vendored copy like any other diff.

## Configuration (`issues/trck.json`)

Zero config works out of the box. To customize, edit `trck.json`:

```json
{
  "update":      { "repo": "leonkacowicz/trck", "channel": "stable" },
  "statuses":    [ {"name": "backlog", "role": "initial"},
                   {"name": "ongoing"},
                   {"name": "done",    "role": "terminal"} ],
  "aliases":     { "start": "ongoing", "done": "done" },
  "priorities":  ["urgent", "high", "medium", "low", "lowest"],
  "default_priority": "medium",
  "kinds":       ["task", "epic", "bug", "story", "investigation"],
  "resolutions": ["superseded", "wontfix", "duplicate"]
}
```

Statuses are an **ordered, free-form list**; the folders are named after them and `SUMMARY.md`
groups by that order. Semantics attach to **roles**, not names:

- `initial` — where `trck new` lands an issue (and the first move off it stamps `started`).
- `terminal` — entering it stamps `closed` and permits a `--resolution`; leaving it (reopen)
  clears both. Multiple terminal statuses are allowed.

The generic `trck mv NNN <status>` moves between any statuses; `start`/`done` are convenience
aliases resolved through `aliases`. So a repo can use, say, `todo → doing → review → shipped`
and either define its own aliases or just use `mv`.

`priorities` is **ordered by precedence** — first is highest — and that order drives
`list --sort priority`, `ready`, and `next`. The priority `trck new` assigns when you don't
pass `--priority` is set separately by `default_priority` (default `medium`); if omitted it
falls back to the middle of the list.

## Common verbs

`new` · `mv` · `start` · `done` · `set` · `dep` · `label` · `show` · `list` · `ready` ·
`next` · `tree` · `deps` · `check` · `summary` · `normalize` · `install-hook` · `init` ·
`update` · `version`. Run `trck --help` (or `trck <verb> --help`) for details.

`ready` lists issues whose dependencies are all satisfied (add `--next` for just the top
pick); `next` prints the single best issue to work on next; `normalize` rewrites
`index.jsonl` in canonical slim form.

Epics: create an epic with `--kind epic`, attach children with `--parent NNN`; the epic's
rollup `%` is computed from its children and shown in `SUMMARY.md`. (Any issue can be a
parent — `kind: epic` is just a display label.) Filter a list to one epic's children with
`trck list --parent NNN`.

Labels: tag issues with a flat, multi-valued set of free-text labels via
`trck label NNN --add X --remove Y`, then filter with `trck list --label X`. Labels show
up in `show`, `list`, `tree`, and `SUMMARY.md`.

Output is colorized when stdout is a terminal (disable with `NO_COLOR=1`, force with
`FORCE_COLOR=1`); piped/redirected output stays plain for scripts and agents. `trck show`
prints a human-readable summary by default — add `--json` for machine-readable metadata.

## Recommended usage

trck gives you four ways to relate issues — **parent/child**, **labels**, **dependencies**,
and **priorities**. They mean different things; using the right one keeps the tracker honest.

### Parent / child = decomposition, not categorization

Make one issue the **child** of another only when the children are a genuine **break-down of
the parent into sub-tasks** — the parent *is* the sum of its children.

- A parent is **not** a generic bucket of similar tasks. For grouping similar work, use
  **labels** instead.
- A parent should be a **single, clear, achievable goal** that you split into the steps
  needed to reach it.
- **Litmus test:** the parent can be marked *done* exactly when all its children are done. If
  finishing the children wouldn't justify closing the parent, it isn't a parent — it's a label.

### Dependencies = hard ordering (MUST)

A **dependency** encodes that one task *must* be completed before another can be:
`A depends on B` means **B blocks A**. It's a real constraint — `trck ready` and `trck next`
will not surface a task until its dependencies are satisfied.

### Priorities = soft ordering (SHOULD)

A **priority** expresses that a task *should* be done before another — an ordering
preference, not a constraint. Nothing is blocked; it just influences what to pick up next.

> Rule of thumb: decomposition → **parent/child**; "a category of similar things" →
> **labels**; "must come first" → **dependency**; "ought to come first" → **priority**.

## Develop

```bash
python3 -m unittest discover -s tests -v
```

The engine is the single file `./trck` (executable, and importable for tests). Keep it
standard-library only. This repo **self-hosts** its own issues under `./issues/` — browse them
to see `trck` tracking its own roadmap.

Releasing: bump `__version__` in `trck`, commit, tag `vX.Y.Z`, and create a GitHub Release —
that release is the stable channel `trck update` consumes.
