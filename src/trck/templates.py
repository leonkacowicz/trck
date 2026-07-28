from __future__ import annotations
import shutil
from .config import initial_status, is_terminal, status_names
from .constants import die, now_utc
from .graph import Graph
from .index import Ctx, Issue, issue_path

# --------------------------------------------------------------------------- #
# TEMPLATE, parse_ids, move_issue, cmd_new, cmd_mv
# --------------------------------------------------------------------------- #

TEMPLATE = """# {title}

## Summary
<!-- What needs doing and why. For an epic, link the spec instead of re-narrating it. -->

## Acceptance criteria
- [ ]

## Notes
<!-- Context, links to files/commits, open questions, decisions. -->
"""

CLAUDE_MD_TEMPLATE = """# Issues — agent & contributor instructions

This folder is an in-repo issue tracker managed by `trck`. **Bookkeeping is scripted;
prose is hand-authored.** You only ever hand-edit the **body** of an issue markdown file
(Summary / Acceptance criteria / Notes). Every structured change — create, move status,
set priority/parent/deps, add labels — goes through `trck`, which updates `index.jsonl`,
regenerates `SUMMARY.md`, and self-validates.

> Never hand-edit `index.jsonl` or `SUMMARY.md`, and never move/rename issue files by hand.

## Where things live
| Data | Source of truth | Changed by |
|---|---|---|
| status | the folder the file is in (configured in `trck.json`) | `trck mv` / `start` / `review` / `done` (moves the file) |
| other metadata | `index.jsonl` (one JSON object per issue) | `trck set` / `dep` / `label` |
| narrative | the issue markdown body | **you** (hand-edited) |
| rollup | `SUMMARY.md` (generated) | `trck` (auto, every mutating verb) |

## IDs
Each issue is keyed by a short **random alphanumeric** id (7 chars from a look-alike-free
base32 alphabet, e.g. `k3m9x2a`) — **not** a sequential integer. Ids are random, so listing
order is *creation* order, not id order. Anywhere a command wants an `ID`, **any unambiguous
prefix works** (`trck show k3m` resolves `k3m9x2a`); an ambiguous prefix errors and lists the
candidates. Legacy integer-id trackers keep working, and `trck renumber` migrates them — a
renumbered issue records its old number in `legacy_id`, so stale `#NN` references still resolve.

## Common verbs (run `trck --help` for all)
Run from anywhere in the repo — `trck` finds the tracker by walking up to the folder
holding `trck.json` (override with `--dir PATH` or `$TRCK_DIR`).

- `trck new "<title>" [--priority …] [--kind …] [--parent ID] [--depends a,b]`
- `trck mv ID <status>` (vocabulary-agnostic); `trck start ID` / `trck review ID [URL]` / `trck done ID [--resolution …]` (aliases)
- `trck set ID [--priority …] [--parent …|none] [--kind …] [--title …] [--pr URL|none] [--field k=v] [--unset k]`
- `trck dep ID --add ID2 | --remove ID2`
- `trck label ID --add X --remove Y`
- Custom fields: `trck set ID --field assignee=leon`; filter `trck list --field assignee=leon`; sort `--sort field:assignee`; show `--show-field assignee`.
- Pull requests: `trck review ID https://…/pull/12` moves to the review status **and** links
  the PR in one step. An issue there is out of `ready`/`next` (nothing to pick up) but still
  **blocks** whatever depends on it until the PR lands. `trck set ID --pr none` unlinks.
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

Rule of thumb: decomposition → **parent/child**; "category of similar things" → **labels**;
"must come first" → **dependency**; "ought to come first" → **priority**.

## Commit hygiene
Keep issue-tracker commits separate from code commits where reasonable. The moved file,
`index.jsonl`, and `SUMMARY.md` are one tracker change and belong together.

## What NOT to do
- Never hand-edit `index.jsonl` or `SUMMARY.md`, and never move or rename issue files by
  hand — the verbs do that.
- Never delete an issue file — close it with `trck done ID --resolution …` so the outcome
  is recorded without losing history.
- Don't put status or priority into the markdown body — that metadata lives in the index.
"""

README_TEMPLATE = """# Issues (managed by `trck`)

This directory is a self-contained, in-repo issue tracker managed by `trck`. `trck.json`
configures it; `index.jsonl` holds metadata; `SUMMARY.md` is generated.

Run `trck --help` for the full verb list — use the vendored `./trck` in this folder if present,
or a `trck` installed on your `PATH`. See `CLAUDE.md` in this folder for the working model.
"""


def parse_ids(spec):
    if not spec:
        return []
    return [p.strip() for p in spec.split(",") if p.strip()]


def guard_dep_edge(g: Graph, src: int, dep: int) -> None:
    """Reject a candidate dependency edge src->dep that violates the disjoint-subtree
    invariant or would close an effective (lifted) dependency cycle. Dies with a
    message naming the offending relationship. Interactive mutators call this before
    `finalize`, so nothing is persisted on rejection."""
    rel = g.containment(src, dep)
    if rel == "same":
        die(f"#{src} cannot depend on itself")
    if rel == "descendant":
        die(f"#{src} is a descendant of #{dep}; a node can't depend on its own "
            f"ancestor (their subtrees overlap)")
    if rel == "ancestor":
        die(f"#{src} is an ancestor of #{dep}; a node can't depend on its own "
            f"descendant (their subtrees overlap)")
    if g.would_cycle(src, dep):
        die(f"#{src} -> #{dep} would create an effective dependency cycle "
            f"(a dependency inherited through the parent hierarchy closes the loop)")


def guard_effective_acyclic(ctx: Ctx, rows: list[Issue]) -> None:
    """Reject a candidate state (rows already mutated in memory) whose effective
    dependency graph has a cycle — the whole-graph source of truth used by hierarchy
    changes, which have no single edge to check. Dies before `finalize` persists."""
    g = Graph(ctx.cfg, rows)
    cycles = g.effective_cycles()
    if cycles:
        die("this change would create an effective dependency cycle: "
            + g.describe_effective_cycle(cycles[0]))


def move_issue(ctx: Ctx, row: Issue, new_status: str) -> None:
    """Move the file between status folders and stamp dates from roles."""
    if new_status not in status_names(ctx.cfg):
        die(f"unknown status '{new_status}' (configured: {', '.join(status_names(ctx.cfg))})")
    old_status = row.status
    old = issue_path(ctx, row)
    row.status = new_status
    new = issue_path(ctx, row)
    if old.resolve() != new.resolve():
        new.parent.mkdir(parents=True, exist_ok=True)
        if not old.exists():
            die(f"file missing for #{row.id}: {old}")
        shutil.move(str(old), str(new))

    init = initial_status(ctx.cfg)
    if old_status == init and new_status != init and not row.started:
        row.started = now_utc()
    if is_terminal(ctx.cfg, new_status):
        if not row.closed:
            row.closed = now_utc()
    else:
        row.closed = None
        row.resolution = None


