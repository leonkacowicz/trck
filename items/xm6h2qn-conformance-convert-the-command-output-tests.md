# conformance: convert the command-output tests

## Summary
Everything a user reads on stdout: `list`, `tree`, `show`, `ready`, `next`, `changelog`, plus
error messages and exit codes. Around 200 call sites drive a command end to end today.

Measured surface (`ast` inventory over the candidate suites, 2026-08-05): **223 command-output
tests carrying 404 assertions**, against 65 genuinely-internal tests that stay in Python.

| suite | cmd-output tests | assertions |
|---|---:|---:|
| test_read.py | 81 | 153 |
| test_json_output.py | 34 | 63 |
| test_metadata.py | 30 | 46 |
| test_review.py | 21 | 42 |
| test_custom_fields.py | 19 | 26 |
| test_labels.py | 9 | 13 |
| test_list_default_filter.py | 7 | 18 |
| test_lifecycle.py | 7 | 15 |
| test_presentation.py | 6 | 11 |
| test_list_progress.py | 4 | 7 |
| test_changelog.py | 3 | 7 |
| test_cli.py | 2 | 3 |

Too large for one sitting, so it is split into children by command surface — see the child
issues. This one is done when they all are.

## Acceptance criteria
- [ ] Each converted case is a fixture directory; the Python original is deleted, not left to
      rot alongside.
- [ ] Error paths and exit codes covered, not just happy paths — an engine that fails
      differently is as wrong as one that succeeds differently.
- [ ] Filter and flag combinations preserved: status/priority/label/kind/parent/match/blocked/
      orphan/field, `--flat`, `--all`, `--sort`, `--show-field`, `--paths`.
- [ ] No loss of coverage: the count of assertions carried over is checked, not assumed.

## Notes
**`test_help.py` is deliberately out of scope.** The Rust engine's `--help` is a stub that says
the port is in progress; help text is not a conformance target and those 12 tests stay in Python
until cutover (#djx63gk).

A golden `expected.out` is strictly stronger than the `assertIn` checks it replaces, so one
fixture can retire several assertions — but every distinct (setup, flag) scenario must still be
represented. That is what "no loss of coverage" has to mean here; a raw fixture-vs-test count
would be the wrong measure.
