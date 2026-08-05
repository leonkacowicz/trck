# rust: validation-error order differs after a mutating verb re-sorts extras

## Summary
When an index row carries invalid non-string custom fields in a non-alphabetical key order,
and a mutating verb (`set`, etc.) rewrites the index — canonicalisation sorts the extra keys —
the two engines report the resulting validation errors in **different orders**:

- **Python** reports them in the row's *original* index order.
- **Rust** reports them in the *canonical* (alphabetically-sorted) order the verb just wrote.

Both flag the same fields with identical wording; only the line order differs. With extras already
in sorted order (or on a read-only `check`), the engines agree — so it is specifically the
mutate-then-report path.

## Acceptance criteria
- [x] The two engines emit multi-field custom-field validation errors in the same order on the
      mutate-then-report path.
- [x] A conformance fixture pins that order (belongs with the error-path cases of #xm6h2qn).

## Notes
Low priority: only reachable with already-invalid data (custom fields must be strings), and the
divergence is purely cosmetic (line order of error output). Decide which order is canonical —
reporting in the stored/original order is arguably friendlier than the post-sort order. Surfaced
while building the #av3efth deps/artifact fixtures via `run.py --compare-bin` on
`index-keeps-an-empty-string-custom-field` (before it was narrowed to the valid empty-string case).

## Resolution
Adopted the **Rust** engine's order (sorted by key) and changed the Python reference to
match, rather than the other way round. The reason: these diagnostics are emitted *after*
the verb has already rewritten the index, and canonical form sorts the extras — so sorted
order is the order of the file the reader is about to open, while insertion order described
a file that no longer existed. It is also input-independent, where insertion order varied
with how the row happened to be typed.

Pinned by `error-custom-field-validation-is-reported-in-key-order`, which seeds three invalid
fields in an order that is neither alphabetical nor the write-back order.
