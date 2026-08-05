# rust: diff and changelog

## Summary
`diff` compares tracker state between revisions over a VCS-agnostic source seam with git as a
layer on top; `changelog` reports what shipped since a date.

Half of `diff` is still unbuilt in Python — four of its six children are open. Worth deciding
whether to port what exists or build the remainder directly in Rust, rather than doing it twice.

## Acceptance criteria
- [x] The source seam and change model, git layer included: revision specs and a HEAD default.
- [x] Whichever layouts have landed at porting time.
- [x] `changelog` since a date or timestamp.
- [x] A recorded decision on porting versus finishing in Rust, and `u5fc5vm` updated to match.

## Decision: port what exists, do not build the remainder in Rust first
`90eba50`. The issue asked for this to be settled, and it is: **port what Python has**.

Four of `u5fc5vm`'s six children are unbuilt, so the tempting move is to build them in Rust
and skip doing the work twice. The argument against is the one thing this whole port rests
on: **verification is differential against the Python engine**, and a feature Rust has that
Python does not has no oracle. That is not theoretical — the differential sweeps have caught
a real bug in nearly every issue this session, including three in this one, and several were
invisible to the fixtures and to hand-written tests.

So the sequencing is: finish the port against a complete oracle, cut over, *then* build the
remaining layouts in Rust alone. At that point there is no oracle, but there is also no
second implementation to diverge from — which is a much better position than having neither
guarantee.

`u5fc5vm` updated to say so.

## Landed
The source seam, the change model, the git layer, and the minimal one-line output. Verified
across 39 invocations over both trackers — four changelog cutoffs including a malformed one,
sixteen revision ranges, and `--from` with a file, a directory and a missing path —
byte-identical, exit codes included.

Three bugs the sweep caught, all mine:

**`--from`/`--to` were not in the value-taking flag list**, so their arguments became
positionals and were read as revision specs: `diff --from index.jsonl` reported "unknown
revision".

**A file source is labelled by its own filename**, not the spec that named it — a long
relative path buries the one word identifying the side being compared.

**A missing required option is a usage error (exit 2)**, not an operational failure. It now
goes through the same table as unknown flags and missing positionals.
