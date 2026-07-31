# diff: body-file edits — flag with a ± line count, --body to inline the real git diff

## Summary
Issue prose lives in `items/*.md`, not in `index.jsonl`, so a row-level join misses body edits
entirely — an issue whose Summary was rewritten looks unchanged. That is a real blind spot for the
"what did this branch do?" use case, since bodies are where the thinking lands.

Cheapest honest answer: detect that the body changed and flag it inline with a line count —
`(body ±12)` — without trying to summarise prose. `--body` then shells out to `git diff` for the
matching paths and inlines the real patch.

## Acceptance criteria
- [ ] A body edit is detected for issues whose row is otherwise unchanged, and such issues still
      appear in the diff.
- [ ] The filename tracks the slug, so a title change renames the file — a rename must read as one
      changed issue, not an add plus a delete (use git's rename detection or match by id prefix).
- [ ] `--body` inlines `git diff` output for the changed body paths, respecting the same revision
      spec.
- [ ] The ± count is line-based and cheap; no attempt to interpret prose.

## Notes
- Depends on the change model; this extends it with a per-issue `body` delta rather than adding a
  parallel path.
- Open question: does the flag belong in the default layout, or only under `-v`/`--body`? Lean
  toward showing it by default — a body-only change is otherwise invisible.
