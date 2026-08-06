# Issues — agent & contributor instructions

This folder is an in-repo issue tracker managed by `trck`. Every issue body lives in
`items/`; status is **not** encoded in the path. **Bookkeeping is scripted;
prose is hand-authored.** You only ever hand-edit the **body** of an issue markdown file
(Summary / Acceptance criteria / Notes). Every structured change — create, move status,
set priority/parent/deps, add labels — goes through `trck`, which updates `index.jsonl`,
regenerates `SUMMARY.md`, and self-validates.

> Never hand-edit `index.jsonl` or `SUMMARY.md`, and never move/rename issue files by hand.

## Where things live
| Data | Source of truth | Changed by |
|---|---|---|
| status | `index.jsonl` (one of the four fixed statuses) | `trck mv` / `start` / `review` / `done` |
| other metadata | `index.jsonl` (one JSON object per issue) | `trck set` / `dep` / `label` |
| narrative | the issue markdown body | **you** (hand-edited) |
| rollup | `SUMMARY.md` (generated) | `trck` (auto, every mutating verb) |

## IDs
Each issue is keyed by a short **random alphanumeric** id (7 chars from a look-alike-free
base32 alphabet, e.g. `k3m9x2a`) — **not** a sequential integer. Ids are random, so listing
order is *creation* order, not id order. Anywhere a command wants an `ID`, **any unambiguous
prefix works** (`trck show k3m` resolves `k3m9x2a`); an ambiguous prefix errors and lists the
candidates. Integer ids were an earlier iteration and are no longer supported at all — a
tracker that still has them is refused, with `scripts/renumber.py` in the trck repo as the way out.

## Common verbs (run `trck --help` for all)
Run from anywhere in the repo — `trck` finds the tracker by walking up to the folder
holding `trck.json` (override with `--dir PATH` or `$TRCK_DIR`).

- `trck new "<title>" [--priority …] [--parent ID] [--depends a,b] [--id ID]`
- `trck mv ID <status>`; `trck start ID` / `trck review ID [URL]` / `trck done ID [--resolution …]` (aliases)
- `trck set ID [--priority …] [--parent …|none] [--title …] [--review-url URL|none] [--field k=v] [--unset k]`
- `trck dep ID --add ID2 | --remove ID2`
- `trck label ID --add X --remove Y`
- Custom fields: `trck set ID --field assignee=alice`; filter `trck list --field assignee=alice`; sort `--sort field:assignee`; show `--show-field assignee`.
- Review links: `trck review ID https://…/pull/12` moves the issue to `in-review` **and**
  records the URL in its `review_url` field, in one step. An issue there is out of
  `ready`/`next` (nothing to pick up) but still **blocks** whatever depends on it until it
  is `done`. `trck set ID --review-url none` unlinks.
- `trck list` · `trck tree` · `trck deps ID` · `trck show ID` · `trck check` · `trck summary`
- Machine-readable: `--json` on `list`/`show`/`deps`/`ready`/`next` — one JSON document each.
- `trck repo normalize` — rewrite `index.jsonl` in canonical slim form (no data change)
- `trck repo install-hook` — install the pre-commit consistency hook
- `trck repo setup-git` — **run once per clone.** Writes this folder's `.gitattributes` and
  registers trck's merge drivers in *your* `.git/config`. Git shares `.gitattributes` but never
  the driver commands (that would make cloning remote code execution), so until a clone runs
  this, `index.jsonl` and `SUMMARY.md` conflict like ordinary text. `trck repo merge-index` /
  `merge-summary` are those drivers — git invokes them; you don't call them by hand. The same
  file pins these formats to LF, which the engine writes and compares byte for byte: checked
  out as CRLF, they would be rewritten whole by the first verb that ran.
- `trck repo migrate-layout` — one-shot upgrade of a pre-0.23 tracker whose issue files still sit
  in per-status folders, into the flat `items/` layout. Such a tracker is **refused by every
  verb** (`legacy status-folder layout: …`) until this runs; `--dry-run` previews the moves.
- `trck update` — pull the latest engine from the canonical repo.

The vocabulary is fixed, not configured: statuses run `backlog → ongoing → in-review → done`,
priorities are `urgent`/`high`/`medium`/`low`/`lowest`, and a closed issue may carry one of
`superseded`/`wontfix`/`duplicate` — no resolution means it shipped. Anything finer is a
label or a custom field. `trck.json` holds only the format version and the update channel;
an engine refuses a tracker whose `format` is newer than it understands.

## Recommended usage

Four ways to relate issues — **parent/child**, **labels**, **dependencies**, **priorities** —
each means something distinct. Pick the right one.

- **Parent / child = decomposition, not categorization.** Make an issue a child of another
  only when the children are a genuine break-down of the parent into sub-tasks — the parent
  *is* the sum of its children. A parent is **not** a generic bucket of similar tasks (use
  **labels** for that); it's a single, clear, achievable goal split into the steps to reach
  it. **Litmus test:** the parent can be marked *done* exactly when all its children are done.
  If finishing the children wouldn't justify closing the parent, it's a label, not a parent.
  Children carry **no order** — containment says *what* composes the parent, not *in what
  sequence*; to sequence sub-tasks, chain them with **dependencies** (2nd depends on 1st, …),
  not by their position under the parent.
- **Dependencies = hard ordering (MUST).** `A depends on B` means B *must* be done before A —
  B **blocks** A. `trck ready`/`trck next` won't surface a task until its deps are satisfied.
  Dependencies **climb the hierarchy**, so one edge covers a whole subtree on both ends:
  depending on a parent depends on its **whole subtree** (recursively — a parent is done only
  when its descendants are), and a parent's deps are **inherited by every child**. Put the
  arrow at the right altitude — on the **parent** when the whole parent depends on something,
  on the **specific children** when only they do — and depend on the specific issue you need,
  not its parent unless you need the entire subtree. An issue and its own ancestor/descendant
  can never depend on each other (a cycle; the mutating verbs and `trck check` reject it);
  siblings and cousins can. **Be precise:** an over-broad edge blocks work that is actually ready.
- **Priorities = soft ordering (SHOULD).** A task that *should* be done before another — a
  preference that influences what to pick up next, not a constraint. Nothing is blocked.
  Set a priority on the issue that **carries** the urgency, not on everything it needs:
  `ready`/`next` rank by **demand**, counting what an issue unblocks (its dependents and
  the parents it composes, transitively) alongside its own priority, so a medium task
  blocking an urgent one outranks a high task blocking nothing. A row lifted that way is
  marked `↑<priority>(#id)` naming what drives it. Marking every prerequisite urgent by
  hand only flattens the ordering you were trying to express.

Rule of thumb: decomposition → **parent/child**; "category of similar things" → **labels**;
"must come first" → **dependency**; "ought to come first" → **priority**.

## Commit hygiene
Keep issue-tracker commits separate from code commits where reasonable. `index.jsonl` and
`SUMMARY.md` are one tracker change and belong together — plus the issue body file when you
wrote prose or the title (and so the slug) changed. A plain status move touches only the first
two: the body file stays put in `items/`.

## What NOT to do
- Never hand-edit `index.jsonl` or `SUMMARY.md`, and never move or rename issue files by
  hand — the verbs do that.
- Never sort issue files into per-status folders. Status lives in `index.jsonl` alone and every
  body stays in `items/`; re-creating status folders is the legacy layout, and every verb then
  refuses to run until `trck repo migrate-layout` undoes it.
- Never delete an issue file — close it with `trck done ID --resolution …` so the outcome
  is recorded without losing history.
- Don't put status or priority into the markdown body — that metadata lives in the index.
