# rust: issue model, index I/O and canonical serialisation

## Summary
The foundation everything else needs: the issue record, reading and writing `index.jsonl`, and
the canonical byte-level form.

## Acceptance criteria
- [ ] The issue record with the same required/defaulted split as today.
- [ ] Unknown keys preserved verbatim through a round-trip, matching `Issue.extra`. This is the
      forward-compatibility guarantee the format version rests on.
- [ ] Canonical serialisation byte-identical to the Python engine's — verified by the
      differential runner, not by inspection.
- [ ] Parse failures loud and specific: a row missing a required field or carrying a wrongly
      typed value is not a well-formed issue and must not be guessed at.
- [ ] Id generation matching the existing alphabet and length.
