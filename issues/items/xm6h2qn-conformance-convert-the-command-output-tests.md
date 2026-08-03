# conformance: convert the command-output tests

## Summary
Everything a user reads on stdout: `list`, `tree`, `show`, `ready`, `next`, `changelog`, plus
error messages and exit codes. Around 200 call sites drive a command end to end today.

## Acceptance criteria
- [ ] Each converted case is a fixture directory; the Python original is deleted, not left to
      rot alongside.
- [ ] Error paths and exit codes covered, not just happy paths — an engine that fails
      differently is as wrong as one that succeeds differently.
- [ ] Filter and flag combinations preserved: status/priority/label/kind/parent/match/blocked/
      orphan/field, `--flat`, `--all`, `--sort`, `--show-field`, `--paths`.
- [ ] No loss of coverage: the count of assertions carried over is checked, not assumed.
