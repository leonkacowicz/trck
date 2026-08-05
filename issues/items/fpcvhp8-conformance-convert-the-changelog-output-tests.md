# conformance: convert the changelog output tests

## Summary
`changelog --since` is the release-notes verb. Source: `test_changelog.py` — 18 tests, of which
the `parse_since` half is genuinely internal and stays in Python.

## Acceptance criteria
- [x] Grouping and ordering of shipped issues.
- [x] The `SINCE` cutoff: bare date, full timestamp, and what a malformed one does (error path
      and exit code).
- [x] Resolution handling — superseded/wontfix/duplicate versus a plain ship.
- [x] Parent/child rollup in the output, if any.
- [x] Python originals for the converted cases deleted; `parse_since` unit tests stay.

## Notes
12 fixtures; both engines agreed on all of them first time — the only batch in #xm6h2qn with no
divergence to fix. Ratchet 184 -> 196. **Retired:** 14 Python tests / 22 assertions, plus two
classes left with no tests and the helpers and imports that orphaned.

**The cutoff is `--since`, not a positional.** This issue's summary said `changelog [SINCE]`;
the flag is required and named. Fixtures use the real form.

**Resolution handling is exclusion, not annotation.** Worth stating plainly since the criterion
only said "handling": an issue closed with *any* resolution does not appear in the changelog at
all. A resolution records a close without a delivery, so the release notes are exactly the
plain-closed set.

**Every fixture seeds `initial/index.jsonl` rather than running `done` in setup.** `TRCK_NOW` is
one fixed instant for every command in a fixture, so verb-driven setup closes everything at the
same moment — no ordering, boundary or cutoff case could be expressed that way. Seeding `closed`
directly is the only way to write these, and it is worth remembering for any future date-sensitive
fixture.

**Dropped the `component`-suffix regression guard.** It asserted no `Title (component)` suffix
appears — a legacy field. All 12 fixtures pin the whole `- #id Title` line, so a reintroduced
suffix would fail every one of them; the guard is strictly stronger now than it was.
