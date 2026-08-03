# conformance: fixture format and the Python runner

## Summary
Move the scenario and its expected output out of Python code and into data files, so something
other than Python can run them. This is what turns the suite from a test suite into a
specification, and it is the thing that makes the port verifiable instead of hoped about.

A fixture is a directory:

```
tests/conformance/ready-excludes-unmet-dep/
  setup.json     {"issues": [{"as": "dep",     "title": "Dep"},
                             {"as": "blocked", "title": "Blocked", "depends": ["dep"]}]}
  cmd            ready
  expected.out   ● #{dep} backlog  medium  Dep
```

Plus a runner — perhaps 100 lines — that builds a temp tracker, runs the command and diffs
against the golden file.

## Acceptance criteria
- [ ] A format covering: initial tracker config, issues to create, a command (or sequence), and
      expected stdout, exit code, and optionally `index.jsonl` and `SUMMARY.md`.
- [ ] **Symbolic ids.** Ids are random, so nothing can be hardcoded — issues declare an alias and
      the runner substitutes real ids into `{alias}` before comparing.
- [ ] **Deterministic dates.** `created` lands in output; either freeze the clock or normalise.
- [ ] A Python runner that discovers fixture directories and reports failures as diffs.
- [ ] An update mode that rewrites goldens, so a deliberate change is one command and shows up
      as a readable diff in review.
- [ ] One converted fixture end to end as proof, with its Python original deleted.

## Notes
Depends on the vocabulary being fixed first (`h8nxqx7`, `qgpk65t`) — otherwise every golden
churns when it lands.
