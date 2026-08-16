## Summary

`conformance/fixtures/discovery-walks-past-a-plain-directory` runs `version` and asserts on
its stdout, which is the literal version number. The verb was chosen because it is harmless —
the fixture is about *discovery*, and `version` is just a way to make the engine resolve a
tracker and say something — but the assertion it produced is the one output in the whole
suite that changes on a release.

So v0.30.1 could not go green until the fixture was edited, and neither can any release
after it. The runner already substitutes `<TRACKER>` and `<WORKDIR>` placeholders; the
version has no such escape.

## Acceptance criteria

- Bumping `version` in `Cargo.toml` leaves the conformance suite passing with no fixture
  edit.
- The discovery behaviour the fixture exists to pin — a plain directory keeps walking up —
  is still covered just as tightly.

## Notes

Two shapes, either fine:

- A `<VERSION>` placeholder in the runner, resolved from the binary under test. Keeps the
  fixture as it is and makes `version` usable anywhere.
- Have the fixture run a verb whose output does not move. It only needs to prove discovery
  reached the tracker above; almost anything else does that.

The second is smaller and needs no runner change. The first is worth it only if another
fixture ever wants to assert on the version deliberately.
