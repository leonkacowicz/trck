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

## IDs
Each issue is keyed by a short **random alphanumeric** id (7 chars from a look-alike-free
base32 alphabet, e.g. `k3m9x2a`) — **not** a sequential integer. Ids are random, so listing
order is *creation* order, not id order. Anywhere a command wants an `ID`, **any unambiguous
prefix works** (`trck show k3m` resolves `k3m9x2a`); an ambiguous prefix errors and lists the
candidates. Legacy integer-id trackers keep working, and `trck renumber` migrates them — a
renumbered issue records its old number in `legacy_id`, so stale `#NN` references still resolve.

## Common verbs (run `trck --help` for all)
- `trck new "<title>" [--priority …] [--kind …] [--parent ID] [--depends a,b]`
- `trck mv ID <status>` (vocabulary-agnostic); `trck start ID` / `trck done ID [--resolution …]` (aliases)
- `trck set ID [--priority …] [--parent …|none] [--kind …] [--title …]`
- `trck dep ID --add ID2 | --remove ID2`
- `trck label ID --add X --remove Y`
- `trck list` · `trck tree` · `trck deps ID` · `trck show ID` · `trck check` · `trck summary`
- `trck normalize` — rewrite `index.jsonl` in canonical slim form (no data change)
- `trck renumber` — convert legacy integer ids to random alphanumeric ids
- `trck update` — pull the latest engine from the canonical repo.

Statuses, priorities, kinds, resolutions, and aliases are configured in `trck.json`.

## Recommended usage

Four ways to relate issues — **parent/child**, **labels**, **dependencies**, **priorities** —
each means something distinct. Pick the right one.

- **Parent / child = decomposition, not categorization.** Make an issue a child of another
  only when the children are a genuine break-down of the parent into sub-tasks — the parent
  *is* the sum of its children. A parent is **not** a generic bucket of similar tasks (use
  **labels** for that); it's a single, clear, achievable goal split into the steps to reach
  it. **Litmus test:** the parent can be marked *done* exactly when all its children are done.
  If finishing the children wouldn't justify closing the parent, it's a label, not a parent.
- **Dependencies = hard ordering (MUST).** `A depends on B` means B *must* be done before A —
  B **blocks** A. `trck ready`/`trck next` won't surface a task until its deps are satisfied.
- **Priorities = soft ordering (SHOULD).** A task that *should* be done before another — a
  preference that influences what to pick up next, not a constraint. Nothing is blocked.

Rule of thumb: decomposition → **parent/child**; "category of similar things" → **labels**;
"must come first" → **dependency**; "ought to come first" → **priority**.
