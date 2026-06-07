# Design: `./release` — deterministic release script

**Date:** 2026-06-06
**Status:** Approved, pending implementation

## Goal

Replace the hand-run release routine (bump `__version__` → commit → tag → write
notes → push → `gh release create`) with a single standalone script. The caller
states only the semver bump level; everything else — including release notes — is
derived deterministically from git history and the trck issue tracker.

## Non-goals

- Not part of the `trck` engine. `./release` is a separate executable, never
  imported by `trck`, and may shell out to `./trck`.
- No changelog file is maintained; notes are generated fresh each release from
  the data already in the repo.
- No loose-commit footer: commits touching `./trck` that have no associated
  done-issue are not listed. The operator hand-edits `RELEASE_NOTES.md` if needed.

## Placement & language

- **Location:** `./release` at the repo root (extensionless, executable, parallels
  `./trck`).
- **Language:** Python 3, standard library only — it parses `issues/index.jsonl`
  and `issues/trck.json`, does semver arithmetic, and drives `git`/`gh` via
  `subprocess`. Bash was rejected (JSON parsing).

## CLI

Two phases, matching the "stop for review before anything goes public" decision.

### Phase 1 — `./release {major|minor|patch}`

All steps here are **local and reversible**.

1. **Pre-flight (abort on any failure):**
   - Working tree is clean (`git status --porcelain` empty).
   - Tests green: `python3 -m unittest discover -s tests`.
   - Tracker clean: `./trck check`.
   - State-consistency guard: `__version__` in `./trck` equals the latest `v*`
     tag (`git tag --sort=-v:refname`, first `vX.Y.Z`). If they differ, the repo
     is mid-release or the last release was incomplete — abort and report.
2. **Compute next version** = latest tag with the chosen component bumped
   (minor/patch reset lower components to 0; e.g. `minor` on `v0.3.0` → `0.4.0`).
3. **Bump** `__version__` in `./trck` to the new version.
4. **Commit** with message `Release vX.Y.Z` (touches only `./trck`).
5. **Tag** `vX.Y.Z` at that commit.
6. **Generate notes** (see below) → write `RELEASE_NOTES.md` (gitignored scratch
   file at repo root).
7. **Stop.** Print the generated notes path and the exact next command:
   `./release --publish`.

If the operator dislikes the result, the local state is undone with
`git tag -d vX.Y.Z && git reset --hard HEAD~1` (the script may print this hint).

### Phase 2 — `./release --publish`

The **irreversible, public** step. The target tag is `v` + the current
`__version__` in `./trck` (which phase 1 just set, and which must equal the
latest local tag). Reads `RELEASE_NOTES.md` for the body:

1. `git push origin main`
2. `git push origin vX.Y.Z`
3. `gh release create vX.Y.Z --title "trck vX.Y.Z" --notes-file RELEASE_NOTES.md`

Pre-flight for phase 2: the tag exists locally, `RELEASE_NOTES.md` exists and is
non-empty, and the bump commit is at `HEAD` (or the tag matches `HEAD`).

## Notes generation

Deterministic, flat list with parent rollup.

1. **Window bound:** latest `v*` tag *before* the new one — i.e. the tag the
   pre-flight guard matched (`<last-tag>`).
2. **Closed-this-window set:** load the issue index at the last tag
   (`git show <last-tag>:issues/index.jsonl`) and the current `issues/index.jsonl`.
   An issue is in the set iff its status is a terminal/`done` status now **and**
   it was *not* terminal (or did not exist) at `<last-tag>`.
   - Terminal status is read from `issues/trck.json` (the same terminal-role
     config the engine uses), not hardcoded to the literal `"done"`.
3. **Parent rollup:** drop any issue in the set that has an ancestor also in the
   set. Walk the `parent` chain (parent ids resolved against the current index).
   A closed epic therefore prints once; its closed descendants are folded in.
4. **Render:**
   ```
   ## What's changed

   - #<id> — <title>
   - #<id> — <title>

   Update with `trck update`.
   ```
   Bullets ordered by ascending id.
5. **Empty set:** if nothing closed since the last tag, write a minimal body
   (heading + the `trck update` footer) and warn on stdout — the operator can
   edit before publishing.

## Components (single file, top-down)

- `run(cmd) -> str` — subprocess helper, raises on non-zero with captured output.
- `preflight_release()` / `preflight_publish()` — the gate checks above.
- `latest_tag() -> str`, `current_version() -> str`, `bump(version, level) -> str`.
- `closed_since(last_tag) -> list[Issue]` — index-diff + rollup; returns ordered
  surviving issues.
- `render_notes(issues) -> str`.
- `cmd_release(level)` / `cmd_publish()` — the two phase orchestrators.
- `main()` — argparse: positional `{major,minor,patch}` XOR `--publish`.

## Testing

- `tests/test_release.py`, using `tests/helpers.load_trck`-style import of the
  extensionless `release` file. Unit-test the pure functions against synthetic
  inputs (no network, no real tag pushes):
  - `bump()` across all three levels and component resets.
  - `closed_since()`: feeds two in-memory index snapshots + a trck.json terminal
    config; asserts the diff set and the parent-rollup suppression (multi-level
    ancestor chain).
  - `render_notes()`: ordering, empty-set body, footer.
- The git/`gh` side effects (`cmd_release`/`cmd_publish`) are driven through the
  `run()` seam so tests can stub it; no test pushes or tags the real repo.

## .gitignore

Add `RELEASE_NOTES.md` (generated scratch artifact).
