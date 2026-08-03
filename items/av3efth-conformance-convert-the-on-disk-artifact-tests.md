# conformance: convert the on-disk artifact tests

## Summary
The artifacts are as much a contract as stdout, and in one respect more: `Issue.to_canonical`
produces the exact bytes the git merge drivers 3-way merge, so its field order and default
stripping are load-bearing.

## Acceptance criteria
- [ ] `index.jsonl` canonical form: field order, stripped defaults, `extra` keys appended in
      stable order, one row per line.
- [ ] `SUMMARY.md` generated output.
- [ ] Issue filenames and slugs, including what happens when a title changes.
- [ ] The unknown-key round-trip: a row carrying a field the engine does not know survives a
      mutating verb untouched.
- [ ] Merge-driver behaviour, which is the reason the byte-level form matters.
