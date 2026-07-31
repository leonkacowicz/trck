# diff -v: per-issue field-level change blocks

## Summary
The complete, unambiguous view: one block per changed issue, one line per changed field. The
`--patch` end of the gradient — verbose by design, used when the summary layers elide something you
need to see.

```
~ #fkrp9dh trck-html: static HTML SPA to browse issues in a browser
     status    ongoing → done
     priority  medium → high
     parent    (none) → #et9qb2y
     labels    +ui −later
     depends   +#v8tmkrt
```

Added issues print their full initial metadata; removed issues print their last-known state.

## Acceptance criteria
- [ ] Every field in the change model is renderable — including custom fields — with no field
      silently omitted.
- [ ] Field names are aligned in a column; values use the same paint helpers as `show`.
- [ ] `(none)` (or equivalent) for a field appearing/disappearing, so an unset is never confusable
      with an empty string.
- [ ] Composes with the layout flags: `-v` deepens whichever layout is selected rather than
      replacing it.

## Notes
- Depends on the change model. This is the layer that proves the model is complete — if something
  can't be rendered here, the model is missing it.
