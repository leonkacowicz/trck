# rust: issue model, index I/O and canonical serialisation

## Summary
The foundation everything else needs: the issue record, reading and writing `index.jsonl`, and
the canonical byte-level form.

## Acceptance criteria
- [x] The issue record with the same required/defaulted split as today.
- [x] Unknown keys preserved verbatim through a round-trip, matching `Issue.extra`. This is the
      forward-compatibility guarantee the format version rests on.
- [x] Canonical serialisation byte-identical to the Python engine's — verified by the
      differential runner, not by inspection.
- [x] Parse failures loud and specific: a row missing a required field or carrying a wrongly
      typed value is not a well-formed issue and must not be guessed at.
- [x] Id generation matching the existing alphabet and length.

## Landed
`070770c`. `json.rs`, `id.rs`, `issue.rs`, `index.rs` — 37 Rust tests, clippy clean under
the workspace's deny list.

**Byte-identity is verified against real data, not by inspection.** This repo's 195-row
index and the example's 35 rows round-trip byte-identically, as a permanent test: those
files *are* Python's canonical output, since `repo normalize` writes them. A generated
pass over the nasty cases — quotes, backslashes, tabs, C0 controls, DEL, `/`, four-byte
emoji, and numbers from `-0` to `1e+100` — matched Python exactly too. The criterion asked
for the differential runner to prove this; it cannot yet, because no verb is implemented,
so this is the strongest available substitute and it is stronger than reading code.

Three things that were not obvious going in:

**Numbers must keep their source text.** Re-formatting a float is exactly where two
languages disagree (`1e100` vs `1e+100`), and an unknown key carrying one has to survive
a round-trip through an engine that never interprets it. Storing the token sidesteps the
whole class.

**Error wording is contract too.** A fixture asserting stderr should not care which engine
produced it, so the Rust side reproduces Python's messages including how it `repr`s the
offending value — hence a `py_repr` in `issue.rs`.

**The read-time migrations are not optional.** `milestone` to a label and `pr` to
`review_url` are part of reading an index correctly; an engine that skipped them would
silently drop a field rather than merely lag.

Dead code is `#[expect]`ed rather than `#[allow]`ed, so the compiler complains once the
port wires everything up — a better reminder to remove it than a comment.
