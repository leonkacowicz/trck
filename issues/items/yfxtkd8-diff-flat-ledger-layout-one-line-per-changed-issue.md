# diff --flat: ledger layout, one line per changed issue

## Summary
A globally-sorted, one-line-per-changed-issue view — the `--flat` counterpart to the nested default
layout, mirroring how `list --flat` relates to `list`.

Reuses `list`'s column layout, with the static status cell replaced by a transition and a leading
`+ ~ -` gutter sigil:

```
+ #a1b2c3d  new              deps --json: {requires, blocks} cones as JSON        medium
~ #2ry5d58  backlog → done   integration tests: real git merges and rebases       high
~ #fkrp9dh  ongoing          trck-html: static HTML SPA        priority med→high  +label ui
- #x9y8z7w  removed          obsolete thing
```

Best when the change set is wide and shallow, or when piping to `grep`. Its weak spot is an issue
that changed in several ways at once — the tail crowds; `-v` is the escape hatch for those.

## Acceptance criteria
- [ ] Column alignment and id highlighting match `list --flat` (shared helpers, not a second
      implementation of the same row).
- [ ] Sigils and the transition arrow are colour-coded by direction: forward, backward, added,
      removed — and degrade to plain text with `NO_COLOR` / non-tty.
- [ ] Non-status edits render as compact chips after the title; a configurable-ish cap keeps the
      line from wrapping (overflow indicated, not silently dropped).
- [ ] Existing `list` output is unchanged.

## Notes
- Depends on the change model. Shares the transition cell and chip rendering with the default
  rollup layout — whichever lands second should reuse, not re-derive.
