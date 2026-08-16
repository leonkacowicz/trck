# Issues — agent & contributor instructions

**You are reading a file on the `trck-issues` branch. Nothing here is checked out.**

This branch *is* the tracker: its root holds `index.jsonl`, the generated `SUMMARY.md`, and one
markdown body per issue under `items/`. `main` holds the code and no tracker at all. The two never
merge, and no tracker change ever appears in a code diff — which is the whole point of the
arrangement rather than a side effect of it.

So there is no directory to `cd` into and no file to open. Run `trck` from anywhere in a normal
checkout of `main`, with no flags: it resolves this branch by itself, reads what it needs out of
the object store, and — for a write verb — builds a commit here and pushes it. Your branch, index
and working tree are untouched however dirty they are.

**Bookkeeping is scripted; prose is hand-authored.** The only thing you author is the **body** of
an issue (Summary / Acceptance criteria / Notes), through `trck edit ID`, which opens your editor
on it and commits what you write. Every structured change — create, move status, set
priority/parent/deps, add labels — goes through a verb, which updates `index.jsonl`, regenerates
`SUMMARY.md`, and self-validates.

> Never try to hand-edit `index.jsonl` or `SUMMARY.md`, and never move or rename a body by hand.
> If you have this branch checked out somewhere to do that, **detach it** first: a write moves
> `refs/heads/trck-issues` under a live checkout, and `git status` there then shows the write
> inverted — a commit that undoes it is one `git commit -a` away.

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
tracker that still has them is unreadable by this engine.

## Common verbs (run `trck --help` for all)
Run from anywhere in the repo, no flags. Resolution order, most explicit first: `--dir` →
`$TRCK_DIR` → `--ref` → `$TRCK_REF` → a tracker directory in the working tree → this branch.
A directory in the tree beats the ref, deliberately — it is what let this repository move over
in pieces, and it is what makes `--dir` on a scratch tracker still work.

The engine must be one that can read a ref: **v0.30.0 or newer**, or a build from `main`. An
older one looks for a directory, does not find one, and says so.

- `trck new "<title>" (--body TEXT | --body-file PATH | --empty) [--priority …] [--parent ID] [--requires a,b] [--id ID]`
  — say where the prose comes from: inline, from a file (`-` reads stdin), or `--empty` for a
  title-only issue. With none of them and no terminal, `new` refuses rather than filing a body
  nobody wrote, so a script or an agent finds out at the point of the mistake.
- `trck mv ID <status>`; `trck start ID` / `trck review ID [URL]` / `trck done ID [--resolution …]` (aliases)
- `trck set ID [--priority …] [--parent …|none] [--title …] [--review-url URL|none] [--field k=v] [--unset k]`
- `trck dep ID --add ID2 | --remove ID2`
- `trck label ID --add X --remove Y`
- Custom fields: `trck set ID --field assignee=alice`; filter `trck list --field assignee=alice`; sort `--sort field:assignee`; show `--show-field assignee`.
- Review links: `trck review ID https://…/pull/12` moves the issue to `in-review` **and**
  records the URL in its `review_url` field, in one step. An issue there is out of
  `ready`/`next` (nothing to pick up) but still **blocks** whatever depends on it until it
  is `done`. `trck set ID --review-url none` unlinks.
- `trck edit ID` — open the body in `$VISUAL`/`$EDITOR` and commit what you write. Same
  `--body`/`--body-file`/`--empty` alternatives as `new`, for when no editor is wanted.
- `trck list` · `trck tree` · `trck deps ID` · `trck show ID` · `trck check` · `trck summary`
- Machine-readable: `--json` on `list`/`show`/`deps`/`ready`/`next` — one JSON document each.
- `trck sync` — push what could not be pushed. A write that cannot reach the remote still
  **succeeded**: its commit is anchored on the local branch, and the verb says so with
  `(N unpushed changes — run `trck sync`)`. Only a *replay* failure is fatal.
- `trck repo normalize` — rewrite `index.jsonl` in canonical slim form (no data change)
- `trck repo migrate-layout` — one-shot upgrade of a pre-0.23 tracker whose issue files still sit
  in per-status folders, into the flat `items/` layout. Such a tracker is **refused by every
  verb** (`legacy status-folder layout: …`) until this runs; `--dry-run` previews the moves.

**Verbs that a ref-backed tracker cannot answer**, because there are no files on disk:
`trck path`, `trck which`, and `list --paths`. They refuse and name the ref rather than printing
a relative path that reads as real and is not there. To read a body raw, use `trck show ID` or
`git show trck-issues:items/<id>-<slug>.md`.

`trck repo install-hook` and `trck repo setup-git` are for a tracker that is a **directory**.
The hook runs `trck check` before a commit that touches the tracker; no commit in a `main`
checkout can touch this one. The merge drivers resolve `index.jsonl` and `SUMMARY.md` when git
merges them; nothing merges them here — a rejected push is replayed from the commit's `Trck-Op:`
trailer and re-derived against whatever landed first, which is stronger than a textual merge and
is why a contended `done` can close a parent neither writer saw complete.
- `trck version` — what engine this tracker is being driven by. There is no self-update:
  whatever installed `trck` owns the file, so upgrade it the way you installed it.

The vocabulary is fixed, not configured: statuses run `backlog → in-progress → in-review → done`,
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

## Commits
You do not make them. Each verb makes exactly one, here, with a subject that says what it did
(`new #abc1234: Title`, `in-progress #abc1234`, `dep #abc1234 +#def5678`) and a `Trck-Op:`
trailer recording the operation losslessly. `git log --oneline trck-issues` is therefore a
readable changelog of the tracker rather than a wall of "update issues".

The trailer is what makes a rejected push recoverable: the pending commits are replayed onto the
fetched tip in order, re-deriving against the other writer's rows. Nothing is ever force-pushed;
a rejection means someone landed first, which is a normal outcome, not an error.

## What NOT to do
- Never hand-edit `index.jsonl` or `SUMMARY.md`, and never move or rename a body by hand — the
  verbs do that. On this branch that means: do not check it out and commit to it directly.
- Never sort issue files into per-status folders. Status lives in `index.jsonl` alone and every
  body stays in `items/`; re-creating status folders is the legacy layout, and every verb then
  refuses to run until `trck repo migrate-layout` undoes it.
- Never re-create an `issues/` directory in `main`'s tree. A tracker directory *wins* over this
  branch, so the repository would silently go back to an empty tracker while this one kept every
  issue. CI treats any `issues/` path as code for exactly that reason.
- Never delete an issue file — close it with `trck done ID --resolution …` so the outcome
  is recorded without losing history.
- Don't put status or priority into the markdown body — that metadata lives in the index.
