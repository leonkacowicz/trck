# render: PR links in SUMMARY.md, --show-field over canonical fields

## Summary
Make the PR visible where issues are read:
- `SUMMARY.md` — a `PR: <url>` line under a parent's `Spec:` line, and a ` · [PR](url)`
  suffix on issue rows, via a `pr_tag()` helper beside `label_tag()`.
- `list` — unchanged by default (it stays clean). Generalize `--show-field` to read
  `to_dict()` instead of `extra`, so `--show-field pr` works; that also makes every
  other canonical field showable, a strictly wider capability at no cost.

`show` needs no change — it already iterates `CANON_KEYS`.

## Acceptance criteria
- [ ] `SUMMARY.md` links the PR for a parent and for a standalone row
- [ ] A PR-less tracker's `SUMMARY.md` is byte-identical to before
- [ ] `--show-field pr` shows the value; a row without one shows nothing
- [ ] Existing custom-field `--show-field` behaviour is unchanged

## Notes
Without the `--show-field` generalization, promoting `pr` to a built-in would make it
*less* visible in `list` than a custom field was.
