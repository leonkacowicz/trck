---
name: trck
description: >-
  Use trck, the single-file in-repo issue tracker, for ALL task and issue
  bookkeeping — and err toward using it. If ANYTHING suggests the repo tracks
  work with trck — an `issues/index.jsonl`, a `trck.json`, a `trck` executable, a
  vendored `issues/trck`, or an `issues/` dir with a generated `SUMMARY.md` —
  treat this skill as in-scope. Trigger it whenever you create, close,
  re-prioritize, or relate issues; decide what to work on next; break work into
  sub-tasks; or realize during any other task that follow-up work is needed —
  even when the user never says "trck", "issue", or "ticket". Also trigger on
  `/trck`, and on any task-, todo-, backlog-, or "what should I work on next"-
  flavored request in such a repo. When unsure whether it applies, invoke it
  anyway: over-triggering here is cheap and intended, while a missed capture (a
  dropped TODO, a stale tracker) costs more. If the repo isn't tracked yet and
  the user invokes `/trck`, walk them through initializing one. Prefer this over
  ad-hoc TODO comments, scratch notes, or mental tracking.
---

# trck — in-repo issue tracker

`trck` is a single-file, standard-library-only issue tracker whose entire state lives in the
repo: one markdown file per issue plus a generated `index.jsonl` and `SUMMARY.md` under a
tracker dir (usually `issues/`). It is driven entirely by the `trck` CLI. This skill covers
how to invoke it, the mental model, the command surface, and — most importantly — the working
discipline that keeps the tracker trustworthy.

Read this once at the start of a session in a trck repo; you should rarely need `trck --help`
afterward. For a per-verb option you don't find here, `trck <verb> -h` is the source of truth.

> This skill is **intentionally biased toward over-triggering.** If there's any sign the repo
> uses trck, use it — a needless activation is cheap, a missed one (a dropped follow-up, a
> tracker that quietly falls out of date) is not. Don't narrow the description to reduce firing.

## 0. Is this repo tracked yet?

Before anything else, check whether the repo already uses trck. The definitive signal is a
committed **`issues/index.jsonl`** (or, more generally, a **`trck.json`** at the tracker dir —
find it with `command -v trck >/dev/null; ls issues/index.jsonl trck.json issues/trck.json 2>/dev/null`).

- **Already tracked** (an `issues/index.jsonl` / `trck.json` exists) → resolve the engine (§1)
  and get to work. This is the common case.
- **Not tracked, and the user invoked `/trck`** (or asked to start tracking) → set it up. Don't
  silently pick defaults; ask a few questions first, because these choices are hard to change
  later and shape how everyone uses the tracker:
  1. **Tracker directory** — default `issues/`. Accept unless they want another name.
  2. **Vendor the engine, or use a global install?** Default: **vendor** (`trck init` commits a
     copy at `issues/trck`) — it pins the engine version to the repo so it can't drift and works
     in CI with no install. Choose `--no-vendor` only if they'll rely on a `trck` already on
     `PATH`. (This is the same vendored-vs-global choice you resolve at §1 on every later run.)
  3. **Install the pre-commit consistency hook?** (`--hook`) — recommended; it runs `trck check`
     so an inconsistent tracker can't be committed.
  4. **Custom status vocabulary?** Default is `backlog → ongoing → in-review → done`. Most repos keep it;
     if they want different statuses/priorities/kinds, note it — those live in `trck.json` and
     can be edited right after init.

  Then run it from an available engine (a global `trck`, or `./trck` if you're in the trck repo
  itself): `trck init [<dir>] [--no-vendor] [--hook]`. Running `trck init` **requires an existing
  trck engine** — if none is on `PATH` and there's no local copy, tell the user they need to
  obtain the single-file `trck` first (there's nothing to bootstrap from otherwise). After init,
  seed the backlog: create the first issues for the work already known (see §4), and run
  `trck check`.

## 1. Which engine to run (vendored vs. global)

There can be more than one `trck` available. Resolve which to use **once**, at the start, and
reuse it. Prefer a copy committed inside the repo over a global install — the committed copy is
pinned to the version the repo's tracker data expects, so it can't drift.

Resolution order:

1. **Vendored engine next to the tracker config.** `trck init` drops a committed copy at
   `<tracker-dir>/trck` (typically `issues/trck`, sitting beside `issues/trck.json`). If it
   exists, use it: `./issues/trck …` (or `python3 ./issues/trck …` if it isn't executable).
2. **The trck project itself.** If the repo root holds the engine as `./trck` (the canonical
   trck repo, set up with `--no-vendor`), use `./trck …`.
3. **Global install.** Otherwise, if `command -v trck` finds one on `PATH`, use plain `trck`.

A quick resolver you can run at session start:

```bash
# Walk up for a committed engine; fall back to the global one on PATH.
TRCK=""
d="$PWD"
while [ "$d" != / ]; do
  for c in "$d/issues/trck" "$d/trck"; do
    if [ -f "$c" ] && { [ -f "$d/issues/trck.json" ] || [ -f "$d/trck.json" ]; }; then TRCK="$c"; break; fi
  done
  [ -n "$TRCK" ] && break
  d="$(dirname "$d")"
done
TRCK="${TRCK:-$(command -v trck)}"
# If $TRCK isn't executable (vendored copies sometimes lack +x), run it as: python3 "$TRCK"
echo "using: ${TRCK:-<none found>}"
```

Throughout this skill commands are written as `trck …`; substitute the path you resolved. The
tracker **dir** is discovered separately by the engine itself (it walks up for `trck.json`), so
you can run from anywhere in the repo; override with `--dir PATH` or `$TRCK_DIR` if needed.

## 2. The model — four ways to relate issues

Each relation means something distinct. Using the right one is what keeps the tracker honest.

- **Parent / child = decomposition (containment).** A child is a genuine break-down of its
  parent into sub-tasks — **the parent *is* the sum of its children.** Litmus test: the parent
  can be marked *done* exactly when all its children are. A parent's status is **derived** from
  its children (all initial → initial, all terminal → terminal, otherwise active), so you never
  set a parent's status by hand. A generic bucket of similar work is **not** a parent — it's a
  label. **Children carry no order** — containment says only *what composes the parent*, not *in
  what sequence*; to sequence sub-tasks, use dependencies (§5), never their position under the
  parent.
- **Dependency = hard ordering (MUST).** `A depends on B` means B must be done before A — B
  **blocks** A. `ready`/`next` hide a task until its deps are satisfied. Dependencies climb the
  hierarchy (see §5) — this is the subtle, high-value part.
- **Label = category.** A flat, free-text tag for grouping similar work across the tree. Use
  this, not a parent, when items are merely "the same kind of thing".
- **Priority = soft ordering (SHOULD).** A preference for what to pick up next, not a
  constraint. Nothing is blocked by priority.

Rule of thumb: decomposition → parent/child; "a category of similar things" → label;
"must come first" → dependency; "ought to come first" → priority.

## 3. Command reference

IDs are short random alphanumeric strings; **any unambiguous prefix works** (`trck show k3m`
resolves `k3m9x2a`). You hand-edit only an issue's **markdown body** (Summary / Acceptance
criteria / Notes) — never `index.jsonl` or `SUMMARY.md`, and never move/rename issue files by
hand; the verbs do that.

**Create & modify**
- `trck new "<title>" [--priority P] [--kind K] [--parent ID] [--depends a,b] [--points N] [--spec PATH] [--pr URL] [--slug S]`
  — create an issue; prints the new file path. Then open that file and write the body prose.
- `trck set ID [--priority P] [--parent ID|none] [--kind K] [--title T] [--points N] [--spec PATH|none] [--pr URL|none] [--field k=v] [--unset k] [--auto]`
  — edit metadata. `--parent` **re-parents** (guarded against cycles); `--field`/`--unset`
  manage free-form custom fields; `--auto` returns a manually-pinned status to derivation.
- `trck mv ID <status> [--pr URL]` — move to any configured status. Aliases: `trck start ID`
  (→ active), `trck review ID [URL]` (→ the review status, recording the pull request),
  `trck done ID [--resolution superseded|wontfix|duplicate]` (→ terminal).
- `trck dep ID --add ID2 | --remove ID2` — add/remove a dependency edge (ID depends on ID2).
- `trck label ID --add X --remove Y` — manage labels.

**Read & navigate**
- `trck list [ID] [filters] [--all] [--flat] [--sort K] [--show-field F] [--paths]` (alias
  `trck tree`) — the issue forest, children nested under parents; pass an `ID` to root it at that
  subtree. Filters: `--status S` (comma-lists alternatives, leading `!` negates — e.g. `'!done'`),
  `--priority P`, `--label L`, `--kind K`, `--parent ID`, `--match TEXT` (title substring),
  `--blocked` (unmet dep), `--orphan` (top-level only), `--field k=v`. Rows carry dim
  `needs #NNN`/`blocks #NNN` blocking notes; parent rows show a rolled-up %-complete. By default
  settled subtrees are hidden; `--all` includes done work, `--flat` gives a globally-sorted list,
  `--paths` prints file paths for piping into rg/grep/fzf.
- `trck show ID` — full metadata + body.
- `trck ready` / `trck next` — actionable leaves (not done, not blocked by an unmet dep,
  directly or inherited, and not parked in a waiting status like `in-review`). `next` is the
  single best pick; `ready --next` is equivalent.
- `trck deps [ID] [--requires|--blocks|--full] [--omit-done|--include-done-chains]` — the
  dependency DAG as a gutter graph. No ID = whole graph; `ID` = that issue's dependency line.
- `trck tree [ID]` — hierarchy view. `trck path ID` / `trck which FILE` — resolve id↔path.
- `trck changelog [SINCE]` — issues shipped since a date/timestamp (release notes).

**Validate & maintain**
- `trck check` — validate consistency; **nonzero exit on error. Run before every commit.**
- `trck summary` — regenerate `SUMMARY.md`. `trck normalize` — canonicalize `index.jsonl`.
- `trck renumber` — migrate legacy integer ids. `trck install-hook` — pre-commit consistency
  hook. `trck init` — scaffold a tracker. `trck update` — self-update the engine.
  `trck version`.

## 4. Working discipline (the point of this skill)

A tracker is only useful if it reflects reality. These habits are what make trck worth having;
follow them proactively, without being asked.

- **Keep the issue list always up to date.** The moment reality changes, reflect it: `start`
  what you begin, `review ID <pr-url>` the moment a pull request opens (it moves the issue to
  the waiting status *and* links the PR, so the work stops showing up as pickable while still
  blocking its dependents), `done` what you finish (with a `--resolution` if it was superseded,
  won't-fix, or a duplicate — that records the outcome without losing history), re-`set`
  priority/parent when your understanding shifts. A stale tracker silently lies about what's left; an accurate
  one lets `ready`/`next` actually be trusted. `trck check` must pass before you commit.
- **Capture every "this needs doing" as an issue — immediately.** Whenever you (or the user)
  realize work is needed — a bug you noticed in passing, a follow-up a change implies, a rough
  edge, a TODO you'd otherwise drop in a comment — create an issue for it right then. Don't rely
  on memory or scatter TODO comments; a captured issue is discoverable, relatable, and survives
  the session. This is the single most important habit: *understanding that something must be
  done → an issue exists for it.*
- **Before creating an issue, skim what's already there.** Every new issue should be placed,
  not dumped. Run a quick `trck list --match <keywords>` / `trck list --all` and check for:
  1. **A duplicate** — if it already exists, update or comment on that one instead of forking a
     second record of the same work.
  2. **Dependencies** — does this new work require something already tracked (add `--depends`),
     or does existing work now depend on it? Wire the edges so ordering is explicit.
  3. **The right parent** — if this is a sub-task of an existing goal, create it under that
     parent (`--parent ID`) so it rolls up correctly. If it's a peer, leave it top-level.
  A well-placed issue makes the whole graph more useful; an unplaced one is noise.
- **Break non-trivial issues into sub-tasks.** If an issue can't be done "in one go", decompose
  it: make it (or a new epic) the parent and create children for the cohesive steps. Keep
  splitting until each leaf is small enough to finish in one sitting, then stop. Points on leaves
  roll up into the parent's progress. Decomposition turns a vague, intimidating issue into a
  checklist you can actually burn down — and makes dependencies expressible at the right grain.
  The sub-tasks are **unordered**; if they must run in sequence, chain them with dependencies
  (2nd depends on 1st, 3rd on 2nd, …) — nesting under a parent implies nothing about order.
- **Be precise about dependencies, at the right altitude** — see §5.

## 5. Dependency precision — put the arrow at the right height

Dependencies **climb the parent hierarchy**, so a single authored edge covers a whole subtree on
both ends. `ready`/`next` and the inline `needs #NNN`/`blocks #NNN` annotations honour these
**effective** dependencies, not just the edges you literally typed:

- **Depending on a parent depends on its whole subtree.** Because a parent is done only when all
  its descendants are, `A depends on P` makes A wait for *every* issue under `P`, recursively.
- **A parent's dependencies are inherited by its children.** If `P depends on B`, every issue
  under `P` is effectively blocked by `B` too — none of `P`'s work can start until `B` is done.
- **An issue and its own ancestor or descendant can never depend on each other** — that would be
  a cycle (a child would wait for its parent, but the parent isn't done until the child is).
  Siblings and cousins may depend on each other freely. Any edge that would close such a cycle —
  directly or through the hierarchy — is rejected when you add it, and `trck check` flags one
  that arrives via a hand-edit.

Because one edge reaches a whole subtree, **place it deliberately:**

- If a parent **as a whole** can't proceed until another issue is done, put the edge on the
  **parent** — every child inherits it automatically. Don't restate it on each child.
- If **only specific children** need it, put the edge on **those children**, not the parent, so
  you don't needlessly block their siblings.
- On the depended-on side, likewise: depend on the **specific issue** you actually need — reach
  for its parent only when you genuinely need its entire subtree done first.

Precise edges keep `ready`/`next` honest: an over-broad dependency hides work that's actually
actionable, and a missing one surfaces work that isn't. When in doubt, prefer the **narrowest**
edge that's still true.

## 6. Guardrails

- **Only ever hand-edit an issue's body prose.** `index.jsonl`, `SUMMARY.md`, and file
  locations are managed by the verbs. Editing them by hand corrupts the tracker.
- **Never delete an issue file** — close it with `trck done ID --resolution …` so the outcome
  is preserved.
- **`trck check` before committing**, always. Keep tracker commits separate from code commits
  where reasonable (the moved file + `index.jsonl` + `SUMMARY.md` are one tracker change and
  belong together).
- The **vocabulary is per-repo** — statuses, priorities, kinds, resolutions, and aliases come
  from `trck.json`. Read it (or `trck --help`) rather than assuming names like `backlog`/`done`;
  another repo may configure different ones.
