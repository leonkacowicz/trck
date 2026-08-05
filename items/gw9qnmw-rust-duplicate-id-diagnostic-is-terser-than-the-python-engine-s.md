# rust: duplicate-id diagnostic is terser than the Python engine's

## Summary
Both engines refuse an index with a repeated id and exit 1, but the Rust diagnostic drops detail
the Python one carries:

```
Python: error: index.jsonl: ids must be unique, but 1 id(s) are repeated:
          #aaaaaaa appears 2 times (statuses: backlog)
Rust:   error: index.jsonl: duplicate ids
          #aaaaaaa appears 2 times
```

Two differences: the headline states the rule Python-side ("ids must be unique") and counts the
repeats, and the per-id line names the statuses involved — which is the part that actually helps,
since a duplicate usually comes from a bad merge and the statuses say which two rows collided.

## Acceptance criteria
- [ ] The Rust engine emits the same headline and the same per-id detail, statuses included.
- [ ] `error-duplicate-ids-in-the-index` is promoted from exit-code-only to asserting stderr.

## Notes
Low priority: the failure is detected and reported by both engines, and only reachable from a
hand-edited or badly-merged index. Surfaced by `run.py --compare-bin` while converting the
error-path tests (#582stb4); that fixture asserts the exit code only until this is closed, and its
comment says so.

Same shape as [[6vpkjxg]] — a cosmetic diagnostic divergence rather than a behavioural one.
