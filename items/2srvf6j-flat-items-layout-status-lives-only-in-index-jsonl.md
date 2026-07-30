# Flat items/ layout: status lives only in index.jsonl

## Summary
Stop encoding issue status in the filesystem. Every issue body moves from
`<tracker>/<status>/<id>-<slug>.md` into a single flat `<tracker>/items/<id>-<slug>.md`, and
`index.jsonl` becomes the sole source of truth for status.

**The problem.** The path duplicates exactly two mutable fields — status (the folder) and slug
(the filename). That duplication is not free:

- `validate` carries a check whose only job is catching the duplication drifting
  (`#ID index status 'X' != folder 'Y'`).
- Every `start`/`review`/`done` is a **file move**, which git records as a rename. Measured over
  this repo's history: **40 of 106** commits touching `issues/` carry renames — **122 renames
  total**, essentially all content-free position changes. That is ~38% of the noise in
  `git log --stat -- issues/` conveying zero information.
- Renames are the merge hazard. Branch A runs `trck start #x` while branch B runs
  `trck set #x --slug …` → rename/rename(1to2) conflict on the body file, *plus* a duplicated
  row for `#x` under the `merge=union` plan in #ey2aruc. That union strategy is only sound once
  bodies stop moving.
- Rename detection is heuristic: a commit that both moves and edits a body can fall under git's
  similarity threshold, showing as delete+add and breaking `git log --follow` on that issue.

**What the folders bought, and why it is affordable to lose.** Zero-tool browsing
(`ls issues/ongoing`, GitHub's tree view) and status-scoped grep. The first is strictly worse
than `SUMMARY.md`, which is generated, always in sync, GitHub-rendered, and carries titles,
priority, and hierarchy. The second is recoverable via `trck list --paths` piped into grep. And
"tool-less" was never a hard constraint anyway — `trck init` vendors the engine into the repo,
so `./issues/trck` is always right there.

## Design

**Chosen shape:** flat `items/`, keyed by `<id>-<slug>.md`.

The path still encodes id and slug, so:
- `trck set --slug` **still renames** the body file — the rare, deliberate case.
- `start`/`review`/`done` stop touching the working tree entirely; a status change becomes a
  one-line `index.jsonl` diff plus the `SUMMARY.md` regeneration.

**Rejected alternatives:**
- *Trie / id-sharded (`items/ab/c1234.md`, git-object style).* Git shards `.git/objects` for
  directory **cardinality** — millions of loose objects. This tracker has ~160 files. Sharding
  buys nothing at that scale and costs all human legibility. The property worth taking from git
  is the **immutable, id-derived path**, not the two-char split.
- *Flat, id only (`items/abc1234.md`).* Fully immutable — even a slug change would not rename.
  Rejected: the filename stops telling you what an issue is, which is most of what makes
  `ls`/grep over the tracker useful. Slug changes are rare enough that the rename is cheap.
- *Bodies folded into `index.jsonl` or one file.* Kills the "hand-edit the body in your editor"
  ergonomics and concentrates every conflict onto one file.
- *Keep folders, add `check --fix`.* Zero format churn, but leaves the merge-conflict problem —
  the thing actually worth fixing — fully intact.

**Blast radius.** Nearly everything funnels through three functions in `src/trck/index.py`
(`filename`, `issue_path`, `rel_link`) plus `scan_files` in `src/trck/scan.py`. Test coupling to
the literal layout is light: two direct path references.

**Back-compat.** This is a breaking on-disk format change, and `trck update` replaces the engine
in place — so a new engine will meet old-layout trackers. Every command detects the old layout
and refuses with one actionable message; a one-shot `trck migrate-layout` verb relocates the
files. Without that guard, `check` would print two useless errors per issue.

**Not reserved:** a status named `items` stays **legal**. Statuses no longer name directories,
so there is nothing to collide with. The only thing that could break is `detect_legacy_layout`,
which globs `<tracker>/<status>/*.md` — it skips the items dir so it cannot mistake the body
directory for a status folder.

## Acceptance criteria
- [ ] Issue bodies live in `<tracker>/items/`; no status folders remain
- [ ] `start`/`review`/`done` produce zero file moves; `set --slug` still renames
- [ ] `validate` no longer carries an index-status-vs-folder check
- [ ] A legacy-layout tracker is detected and refused with an actionable message
- [ ] `trck migrate-layout` migrates a tracker; `trck check` passes afterward; idempotent
- [ ] A status named `items` remains legal and never self-detects as legacy
- [ ] This repo's tracker and `examples/action-game` are migrated
- [ ] README, `CLAUDE.md`, and `CLAUDE_MD_TEMPLATE` no longer describe status as a folder
- [ ] Released as v0.23.0 with the breaking change led in the release notes

## Notes
Full implementation plan with exact code and per-step TDD cycles:
`docs/plans/2026-07-30-flat-items-layout.md`.

Sub-tasks are chained by dependency because they are genuinely sequential — nesting alone
implies no order.

**Related, not blocking:**
- #ey2aruc (merge drivers) becomes tractable once this lands — removing renames leaves only the
  `index.jsonl` union to solve. No hard ordering, so no dependency edge.
- #s3d6xyz (`reconfigure` to rename/reorder statuses) gets substantially cheaper: renaming a
  status becomes an index rewrite instead of a folder move.
- #qs4zwzr (grouping the maintenance verbs) may change where `migrate-layout` is born — decide
  it before the migrate-layout sub-task lands, or accept renaming the verb later.
