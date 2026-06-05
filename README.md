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
curl -fsSL https://raw.githubusercontent.com/leonkacowicz/trck/v0.1.2/trck \
  -o ~/.local/bin/trck && chmod +x ~/.local/bin/trck
```

Then, in any repo:

```bash
trck init                       # scaffold ./issues (config + a vendored copy of trck)
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
  "priorities":  ["high", "medium", "low"],
  "kinds":       ["task", "epic"],
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

## Common verbs

`new` · `mv` · `start` · `done` · `set` · `dep` · `rename` · `show` · `list` · `tree` ·
`deps` · `check` · `summary` · `install-hook` · `init` · `update` · `version`. Run
`trck --help` (or `trck <verb> --help`) for details.

Epics and milestones: create an epic with `--epic`, attach children with `--parent NNN`
(and optionally `--milestone M1`); the epic's rollup `%` is computed from its children and
shown in `SUMMARY.md`.

## Develop

```bash
python3 -m unittest discover -s tests -v
```

The engine is the single file `./trck` (executable, and importable for tests). Keep it
standard-library only. This repo **self-hosts** its own issues under `./issues/` — browse them
to see `trck` tracking its own roadmap.

Releasing: bump `__version__` in `trck`, commit, tag `vX.Y.Z`, and create a GitHub Release —
that release is the stable channel `trck update` consumes.
