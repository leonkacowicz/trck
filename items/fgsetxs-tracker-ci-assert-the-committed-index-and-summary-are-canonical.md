# tracker CI: assert the committed index and summary are canonical

## Summary

Nothing checks that `index.jsonl` and `SUMMARY.md` on the tracker branch are what the engine
would write. `trck check` validates the tracker's *content* — dangling parents, cycles, a row
with no body — but not that the two generated files are in canonical form. A hand-edit, a
bad merge, or a write from an engine of a different vintage can leave them subtly off with
everything still passing.

This used to be covered by accident: a unit test parsed the committed `index.jsonl` and
required `render_index` to give the bytes back. That was never really a unit test — the
fixture was live data and a failure meant "run `trck repo normalize`", not "there is a bug"
— and it went away with the flip. The check itself is worth having; it just belongs
somewhere else.

## Where it belongs

`.github/workflows/tracker.yml`, on the branch, beside the `check` it already runs. That
workflow fetches the pushed commit and already has an engine built from `main`, so the
marginal cost is one step. And it fires on every tracker commit, which is far more often
than a unit test that only ran when someone touched the engine.

## What it needs

No verb answers the question today. `trck repo normalize` rewrites `index.jsonl` in place
and reports how many rows changed; `trck summary` regenerates `SUMMARY.md`. Neither has a
mode that compares, writes nothing, and exits non-zero.

Sketch: `trck repo normalize --check`, covering both generated files — they are one notion of
"canonical" and splitting them across two flags would invite checking one and not the other.
It reports the first file that differs and how, writes nothing, and exits non-zero.

Worth deciding: whether this should instead be folded into `trck check`. Against — `check`'s
meaning would change for every existing user, and a stale `SUMMARY.md` is a different class
of problem from a dependency cycle: one is a regenerable artefact, the other is a broken
tracker. For — it is one fewer thing to remember to run.

## Acceptance criteria
- [ ] A verb answers "the committed index and summary are what the engine would write", writing nothing and exiting non-zero when they are not.
- [ ] Its diagnostic names which file differs and gives enough to act on.
- [ ] `.github/workflows/tracker.yml` runs it against the pushed commit, beside `check`.
- [ ] Conformance covers the clean case and each dirty case.

## Notes

Split from #r26hw48, which kept the engine-testing half. Do not merge them back: one is a
test of the engine against a fixture that never changes, the other is a check of data that
changes on every commit.
