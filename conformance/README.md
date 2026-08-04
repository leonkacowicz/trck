# trck conformance suite

Executable specification for a trck implementation. Each fixture is a directory: some
initial state, one command, and what that command is supposed to produce.

```bash
python3 conformance/run.py                        # against ./trck
TRCK_BIN=target/release/trck python3 conformance/run.py
python3 conformance/run.py -k gutter              # a subset
python3 conformance/run.py --update               # accept new output as correct
```

**The runner never imports trck.** It execs whatever `TRCK_BIN` points at. That is the
whole reason this exists — a runner that imported the Python engine would silently
become Python-only the moment a second implementation arrived, and nothing would fail to
tell you.

## A fixture

```
fixtures/deps-lone-blocker-slides-down/
  setup                 commands that build the starting state (optional)
  cmd                   the one command under test (required)
  expected.out          golden stdout
```

Every file except `cmd` is optional, and **absent means not asserted**. A fixture states
what it cares about and stays silent on the rest, so adding a column to an unrelated
view does not churn every golden in the suite.

| file | what it does |
|---|---|
| `initial/` | copied over the fresh tracker before anything runs |
| `env` | `KEY=VALUE` lines added to the environment of every invocation |
| `setup` | one command per line, run in order; a non-zero exit aborts the fixture |
| `cmd` | exactly one command line — the thing under test |
| `expected.out` | stdout, compared literally |
| `expected.err` | stderr, compared literally |
| `expected.code` | exit status. **Asserted as 0 when absent** — a fixture that forgets to mention it still means "this is supposed to work" |
| `expected.index.jsonl` | the tracker's index after the command |
| `expected.SUMMARY.md` | the generated summary after the command |

`setup` and `cmd` lines are split like a shell (`shlex`), so a quoted title stays one
argument. Lines starting with `#` are comments — use them; a fixture whose name is its
only explanation is a fixture nobody dares change.

## Two things make this reproducible

**Ids are chosen, not generated.** Setup uses `new --id aaaaaaa`, so a golden can name
an id, filenames are stable, and tie-breaks that fall through to id ordering do not flap.
The alternative — substituting placeholders into goldens afterwards — is lossy in a way
that matters here: if a command emits two ids *swapped*, normalising them by first
appearance renames them to match and the fixture passes.

**The clock is fixed.** Every invocation runs with `TRCK_NOW=2026-01-01T00:00:00Z`, so
`created`/`started`/`closed` are the same on every run and `expected.index.jsonl` can be
compared byte for byte. The engine's own clock override is part of the contract, not a
Python-side hook.

Output is also run with `NO_COLOR=1`, and the tracker's temp path is replaced with
`<TRACKER>` before comparison.

## Setup: commands or literal state

Prefer `setup` commands. They express the scenario in the vocabulary a reader already
knows, and they stay correct when the on-disk format changes.

Use `initial/` for states the verbs cannot produce — a row carrying a field this engine
has never heard of, a deliberately malformed row, a legacy shape, a specific `trck.json`.
`unknown-index-keys-round-trip` is the worked example: it plants an unknown key, runs an
ordinary `set`, and asserts the key survived.

## Adding a fixture

1. `mkdir fixtures/<name>` and write `cmd` (plus `setup` if it needs a starting state).
2. `python3 conformance/run.py --update -k <name>` — a fixture with nothing asserted yet
   gets its stdout captured, plus stderr and exit code when they are interesting.
3. **Read the golden.** It is the assertion; `--update` only wrote down what happened,
   not what should have.

To start asserting something else, create the file and re-run with `--update`:

```bash
touch fixtures/<name>/expected.index.jsonl
python3 conformance/run.py --update -k <name>
```

`--update` deliberately refreshes only goldens that already exist. Creating them all
would make every fixture assert everything on its first run, which is exactly the
property this format is built around.

## Measuring a half-finished implementation

```bash
python3 conformance/run.py --bin target/release/trck --min-pass 0
```

`--min-pass N` succeeds as long as at least N fixtures pass. It is a **ratchet, not a
mute button**: an engine mid-port is expected to fail most fixtures, but "most" has to be
a number that only goes up. Without a floor the job is either permanently red and
therefore ignored, or green and therefore meaningless. When more pass than the floor, the
runner says so and asks you to raise it — one visible commit per step forward.

## Differential mode

```bash
python3 conformance/run.py --compare-bin target/release/trck
```

Runs every fixture against **two** binaries and diffs them against each other rather than
against the goldens. This is the oracle for the port: it answers "do these two agree" for
cases nobody wrote a fixture for, so a disagreement can be caught before anyone has
decided what the right answer is.

## What belongs here

Anything a user or a downstream tool would notice: command output, exit codes, the
on-disk artifacts, the ordering of ranked results, the shape of `--json`. Not the
internals that produce them — a second implementation is free to compute a ranking
differently as long as the order matches.

## Not yet expressible

- **A moving clock.** `env` applies to the whole fixture, so "created Monday, closed
  Friday" cannot be written. The engine reads `TRCK_NOW` per invocation, so the missing
  piece is per-line environment in the format, not in the engine.
- **A command sequence under test.** `cmd` is one command by design; multi-step cases are
  written as literal setup plus the one step that matters.
