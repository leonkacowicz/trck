# Tracker format and compatibility

How a tracker on disk is versioned, what an engine does when it meets one it does not
understand, and which older shapes are still read. See the README for the day-to-day
contents of `issues/trck.json`.

## Format version

`format` says which shape the tracker is written in. An engine **refuses a tracker newer than
it understands** — every verb goes through one guard, so there is no path that reads or writes
a layout it can only half-parse. It never refuses an *older* one; that is what the migration
verbs are for. Omitting the key means "the current shape", so every tracker written before this
existed keeps working. The refusal names the remedy — upgrade the binary — since the engine
has no way to migrate a shape it was written before.

Bumps are rare, because the test is whether an older engine would be **wrong**, not merely
ignorant:

| change | bump? |
|---|---|
| a new field in `index.jsonl` | **no** — unknown keys round-trip verbatim, so an old engine preserves it |
| a new verb, flag, or column | **no** |
| an existing field changing meaning, or data moving | **yes** — an old engine gives wrong answers or destroys data |
| an opt-in feature only some trackers use | **neither** — that is an extension |

## Extensions

Extensions are git's model, taken for its granularity. A flat version pins the whole tracker,
so bumping it for an opt-in feature would lock out old engines for every repo, including the
ones not using it:

```json
{ "format": 1, "extensions": { "some-feature": {} } }
```

The version means "you may meet extension keys — refuse any you do not know", so only the
repos that opted in are affected. No extensions are defined yet.

One honest limit: this protects engines from the release that introduced it onward. An engine
predating it ignores both keys and can still be fooled, so the guard is a floor rather than a
guarantee — keep everyone reading a shared tracker on a version that has it.

## Shapes still read

- **Old vocabulary keys in `trck.json`.** The vocabulary used to be configured per tracker.
  A tracker still carrying those keys is not broken: they're ignored, and `check` names each
  one.
- **`pr` instead of `review_url`.** A tracker written before the rename carries `pr`; it is
  migrated on read and rewritten on the issue's next mutation, so nothing breaks and no
  migration verb is needed.
- **Sequential ids.** Ids used to be sequential integers, which collided when two branches
  each ran `trck new`. Current ids are random (see the README), and `scripts/` carries a
  converter for a tracker still on the old scheme.
- **Status-in-the-path layouts.** Bodies used to live in per-status directories.
  `trck repo migrate-layout` converts a pre-0.23 tracker to the flat `items/` layout; every
  verb refuses one until it runs.

## Pinning the clock

`TRCK_NOW` fixes the timestamp a command stamps into `created`/`started`/`closed`:

```bash
TRCK_NOW=2026-01-01T00:00:00Z trck new "Reproducible"
```

It's read per invocation, so a script can advance it between commands. Any ISO-8601
instant is accepted and normalised to UTC; a malformed or day-only value is an error
rather than a silent fall back to the real clock. This exists so the conformance suite
can compare `index.jsonl` byte for byte — it is part of the specification, not a test hook
bolted onto one implementation.
