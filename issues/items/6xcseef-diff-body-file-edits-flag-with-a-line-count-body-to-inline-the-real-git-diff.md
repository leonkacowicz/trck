# diff: body-file edits — flag with a ± line count, --body to inline the real git diff

## Summary
Issue prose lives in `items/*.md`, not in `index.jsonl`, so a row-level join misses body edits
entirely — an issue whose Summary was rewritten looks unchanged. That is a real blind spot for the
"what did this branch do?" use case, since bodies are where the thinking lands.

Cheapest honest answer: detect that the body changed and flag it inline with a line count —
`(body ±12)` — without trying to summarise prose. `--body` then inlines the real patch, produced
with `difflib` from the two snapshots' body text so it works without git, exactly like the rest of
the diff.

Bodies come through the snapshot seam's `body(id)`, which returns **`None` when the source cannot
supply bodies** (`--from <index.jsonl>`, `--from -`). That is not the same as an empty body and must
not be reported as "unchanged" — an unavailable side means body detection is skipped for that run,
and says so once rather than silently under-reporting.

## Acceptance criteria
- [ ] A body edit is detected for issues whose row is otherwise unchanged, and such issues still
      appear in the diff.
- [ ] The filename tracks the slug, so a title change renames the file — a rename must read as one
      changed issue, not an add plus a delete (use git's rename detection or match by id prefix).
- [ ] `--body` inlines a unified diff built with `difflib` from the two snapshots' body text — no
      shelling out, so it works for every source, not just git.
- [ ] A source that cannot supply bodies skips body detection and reports that once, rather than
      reporting every body as unchanged.
- [ ] The ± count is line-based and cheap; no attempt to interpret prose.

## Notes
- Depends on the change model; this extends it with a per-issue `body` delta rather than adding a
  parallel path.
- Depends on the git provider too, because it is the provider that must fetch bodies at a revision
  (`git show <rev>:<tracker>/items/<file>`) — lazily, so a run without `--body` stays cheap.
- Open question: does the flag belong in the default layout, or only under `-v`/`--body`? Lean
  toward showing it by default — a body-only change is otherwise invisible.
