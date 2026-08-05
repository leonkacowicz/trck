# conformance: convert the mutating-verb output tests

## Summary
What the mutating verbs echo to stdout, which is as much a contract as the read verbs: `new`
(prints the created path), `mv`/`start`/`review`/`done`, `set`, `dep`, `label`. Sources:
`test_metadata.py`, `test_labels.py`, `test_lifecycle.py`, `test_review.py`,
`test_custom_fields.py` — the stdout-asserting cases in each.

## Acceptance criteria
- [x] Each verb's echo line, including the created path from `new`.
- [x] `review` recording the PR URL and moving status in one step; `done --resolution ...`.
- [x] `label --add/--remove` sorted-and-echoed; `set --field`/`--unset`.
- [x] Derived-status behaviour on a parent (`--auto`, and refusing a hand-set status).
- [x] Python originals deleted; assertion count carried over is checked.

## Notes
33 fixtures; both engines agree. Ratchet 112 -> 145. Each fixture asserts the echo line **and**
the row the verb wrote — asserting only one lets the other drift.

**Retired:** 32 Python tests / 54 assertions, plus five helpers and one import left dead.

**A hand-set parent status is pinned, not refused.** The criterion's wording was wrong: `mv` on a
parent succeeds and records `manual_status: true`, which tells the rollup not to overwrite it;
`set --auto` drops the pin. Fixtures capture the real behaviour
(`mv-on-a-parent-pins-its-status`, `set-auto-returns-a-pinned-parent-to-derivation`).

**Found a real Rust bug.** `reopening-clears-closed-and-resolution` was the one fixture the two
engines disagreed on: Rust cleared `closed` but kept `resolution`, leaving a row that is open and
still says why it closed — which its own `check` rejects. Fixed in the Rust engine with a unit
test, ahead of the fixture commit.

**Scope kept off:** error paths (`dep` cycle rejections, bad values, missing args) belong to
#582stb4; field *display* belongs to #wtwzr4s. Legacy `pr`->`review_url` migration and the
milestone->label migration stay in Python — they are load-path concerns, not verb output.
