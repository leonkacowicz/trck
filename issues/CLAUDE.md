# Issues — agent & contributor instructions

This folder is an in-repo issue tracker managed by `trck`. **Bookkeeping is scripted;
prose is hand-authored.** You only ever hand-edit the **body** of an issue markdown file
(Summary / Acceptance criteria / Notes). Every structured change — create, move status,
set priority/parent/deps, add labels — goes through `trck`, which updates `index.jsonl`,
regenerates `SUMMARY.md`, and self-validates.

> Never hand-edit `index.jsonl` or `SUMMARY.md`, and never move/rename issue files by hand.

## Where things live
| Data | Source of truth |
|---|---|
| status | the folder the file is in (configured in `trck.json`) |
| other metadata | `index.jsonl` (one JSON object per issue) |
| narrative | the issue markdown body |
| rollup | `SUMMARY.md` (generated) |

## Common verbs (run `trck --help` for all)
- `trck new "<title>" [--priority …] [--kind …] [--parent NNN] [--depends a,b]`
- `trck mv NNN <status>` (vocabulary-agnostic); `trck start NNN` / `trck done NNN [--resolution …]` (aliases)
- `trck set NNN [--priority …] [--parent …|none] [--kind …] [--title …]`
- `trck dep NNN --add MMM | --remove MMM`
- `trck label NNN --add X --remove Y`
- `trck list` · `trck tree` · `trck deps NNN` · `trck show NNN` · `trck check` · `trck summary`
- `trck normalize` — rewrite `index.jsonl` in canonical slim form (no data change)
- `trck update` — pull the latest engine from the canonical repo.

Statuses, priorities, kinds, resolutions, and aliases are configured in `trck.json`.
