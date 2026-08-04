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
- [x] A format covering: initial tracker config, issues to create, a command (or sequence), and
      expected stdout, exit code, and optionally `index.jsonl` and `SUMMARY.md`.
- [x] **Symbolic ids.** Ids are random, so nothing can be hardcoded — issues declare an alias and
      the runner substitutes real ids into `{alias}` before comparing.
- [x] **Deterministic dates.** `created` lands in output; either freeze the clock or normalise.
- [x] A Python runner that discovers fixture directories and reports failures as diffs.
- [x] An update mode that rewrites goldens, so a deliberate change is one command and shows up
      as a readable diff in review.
- [x] One converted fixture end to end as proof, with its Python original deleted.

## Notes
Depends on the vocabulary being fixed first (`h8nxqx7`, `qgpk65t`) — otherwise every golden
churns when it lands.

## Landed
`d9adfbe`. `conformance/` at the top level — the spec both engines answer to, not a
subdirectory of the Python suite — with `run.py`, `README.md` documenting the format for
whoever writes the Rust side, and four fixtures covering one assertion kind each.

**Symbolic ids were dropped from the plan.** The criterion said aliases substituted into
goldens; `new --id` (`jpash72`) replaced it. Substitution is lossy where it matters: two ids
emitted *swapped* normalise by first appearance to match and the fixture passes. Chosen ids
also fix filenames and the id-order tie-breaks that would otherwise flap.

**`--update` refreshes only goldens that already exist.** The first cut created them all,
which made every fixture assert its whole index and summary on the first run — destroying
the "absent means not asserted" property in one command. A brand-new fixture gets stdout
and nothing more; anything else is opted into by creating the file.

**Exit code is asserted as 0 when unstated**, unlike every other golden. A fixture that
forgets to mention it still means "this is supposed to work".

It found a bug on its first run: `mv` reported an unknown status as `(configured: …)`,
wording left from when the vocabulary was configurable. Fixed in the same commit.

Two things the format cannot express yet, written up in the README rather than left to be
rediscovered: a clock that advances between setup steps (the engine reads `TRCK_NOW` per
invocation, so the gap is per-line environment in the format), and a multi-command
sequence under test.
