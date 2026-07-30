# docs: status is an index field, not a directory

## Summary
Three user-facing documents describe status as the folder a file sits in. Each is now wrong.

## Implementation

Start by capturing every stale reference:
```bash
grep -rn "folder\|backlog/\|ongoing/\|done/" README.md CLAUDE.md issues/CLAUDE.md src/trck/templates.py \
  | tee /tmp/claude-1000/stale-layout-docs.txt
```

**`README.md`** — line 4 currently reads `Status is the folder a markdown file sits in; all other
metadata lives in`. Replace with `Every issue is a markdown file in `items/`; all metadata —
status included — lives in`. Then reword the statuses paragraph near line 85 (`the folders are
named after them`): statuses are an ordered, free-form list naming the values `mv`/`start`/`done`
move between and the sections `SUMMARY.md` groups by — they no longer name directories.

**`src/trck/templates.py`** (`CLAUDE_MD_TEMPLATE`) — the metadata table row:
```
| status | the folder the file is in (configured in `trck.json`) | `trck mv` / `start` / `review` / `done` (moves the file) |
```
becomes
```
| status | a value from `trck.json`, stored in `index.jsonl` | `trck mv` / `start` / `review` / `done` |
```
Leave the *tracker-discovery* sentences alone — "walking up to the folder containing trck.json"
is still correct.

**`CLAUDE.md`** — add to the dogfooding bullet list:
```markdown
- Issue bodies all live in `issues/items/` — status is **not** encoded in the path; it lives
  only in `index.jsonl`. A `start`/`done` touches the index and `SUMMARY.md`, never the body file.
```

**`issues/CLAUDE.md`** — a copy of `CLAUDE_MD_TEMPLATE` written by `trck init`. Apply the same
table edit by hand; regenerating would overwrite this repo's local edits.

Verify nothing survives:
```bash
grep -rn "status is the folder\|the folder the file is in\|moves the file" \
  README.md CLAUDE.md issues/CLAUDE.md src/trck/templates.py
```

## Acceptance criteria
- [ ] README's opening no longer calls status a folder
- [ ] The statuses section no longer says folders are named after statuses
- [ ] `CLAUDE_MD_TEMPLATE`'s metadata table names `index.jsonl` as status's home
- [ ] `CLAUDE.md` and `issues/CLAUDE.md` describe the flat layout
- [ ] The verification grep returns nothing
- [ ] Full suite green (some init tests assert template text)

## Notes
Step-by-step: `docs/plans/2026-07-30-flat-items-layout.md` (Task 6).
