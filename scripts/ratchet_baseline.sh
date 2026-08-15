#!/bin/sh
# Compare the working tree's quality report against a baseline *measured* from <rev>.
#
# `ratchet compare --base <rev>` reads `git show <rev>:quality-report.json`, which is
# whatever ratchet version happened to write it. That is fine until the tool's own metrics
# change — 0.2.0 began counting test code — and then every file reads as a regression
# against an older baseline on a change that touched nothing. Checking the base tree out
# and measuring it with the ratchet that is running now compares like with like.
#
# Both sides keep their own `ratchet.json`, so a threshold edit still reaches `compare`
# and is still refused when it arrives alongside new violations.
#
# One argument: the revision to compare against.
set -eu

rev="${1:?usage: ratchet_baseline.sh <rev>}"
base="${TMPDIR:-/tmp}/ratchet-baseline"

rm -rf "$base"
mkdir -p "$base"
# `git archive` rather than a worktree: no ref to create, nothing to clean up afterwards,
# and it works from a bare-ish fetch of just that revision.
git archive "$rev" | tar -x -C "$base"

ratchet generate --root "$base"
ratchet compare --root . --base-file "$base/quality-report.json"
