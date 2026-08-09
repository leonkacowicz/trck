# validate and issue each carry their own Python repr, and they disagree

## Summary
Two functions render a JSON value the way Python's `repr` would, so that this engine and the
Python one word the same complaint identically:

- `src/issue/diagnostic.rs::py_repr` — used by `from_json`'s type errors.
- `src/validate/row.rs::repr` — used by `check`'s custom-field errors.

They agree on `None`/`True`/`False`, numbers, lists and dicts. They differ on one thing: strings.

```rust
// issue/diagnostic.rs
Json::String(s) => format!("'{}'", s.replace('\\', "\\\\").replace('\'', "\\'")),
// validate/row.rs
Json::String(s) => format!("'{s}'"),
```

So a custom field whose value contains a quote or a backslash is quoted one way by `check` and
another by a parse error. Python's own `repr` escapes, which makes `validate`'s the wrong one.

Noticed while paying down `validate/mod.rs` (`#qct2pvw`) — deliberately **not** fixed there,
because unifying them changes `check`'s output and that belongs in a change whose goldens are
about the wording rather than about a refactor.

## Acceptance criteria
- [ ] One implementation, shared — not two that happen to agree on most inputs.
- [ ] A quote and a backslash in a custom-field value are escaped the way Python escapes them.
- [ ] A conformance fixture covers a custom field carrying a quote, so the wording is pinned
      rather than left to whichever module was reached first.

## Notes
Low priority: it takes a custom field value containing `'` or `\` to see the difference, and the
consequence is a cosmetically wrong diagnostic rather than a wrong decision. It is worth fixing
because "the two engines word it the same" is the entire reason either function exists — a
near-copy that is subtly wrong is worse than no copy, since it looks deliberate.

Where the shared one should live is the open question. `issue::diagnostic` is the older and more
correct of the two, but `validate` depending on `issue`'s private diagnostic module to word its own
errors is a strange edge; a small `py_repr` beside `json` may be the better home, since what it
really renders is a `Json`.
