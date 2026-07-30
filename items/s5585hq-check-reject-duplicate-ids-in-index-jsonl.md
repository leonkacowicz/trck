# check: reject duplicate ids in index.jsonl

## Summary
`trck check` reports a corrupt index as clean. Two rows carrying the same `id` pass validation
with `0 errors`, and the issue then renders twice — in two different statuses.

Reproduced against v0.23.0 with a hand-written index:

```jsonl
{"id": "abc1234", "slug": "alpha", "title": "Alpha", "kind": "task", "status": "ongoing", "priority": "high"}
{"id": "abc1234", "slug": "alpha", "title": "Alpha", "kind": "task", "status": "done", "priority": "high"}
```
```
$ trck check
OK — 2 issues, 0 errors, 0 warning(s)      ← exit 0

$ trck list --all
◐ #abc1234 ongoing  high  Alpha
● #abc1234 done     high  Alpha
```

The engine's only duplicate guard is `scan.py:24`, and it protects **on-disk filenames**
(`duplicate issue id … on disk`), not index rows. `load_index` appends rows without checking,
and everything downstream keys by id into a dict (`Graph.by_id`) — so one row silently wins for
graph purposes while both are counted, listed, and re-serialized.

**Why urgent.** This is the failure mode a `merge=union` strategy on `index.jsonl` produces
(#ey2aruc) — and until v0.23.0 the *old* status-folder layout accidentally masked it. Two
branches both moving one issue also moved its body file into two different folders, so git
raised a rename/rename conflict and a human had to look. Now that a status change is a pure
index edit, that signal is gone: nothing in git or in `check` notices. The tracker's core
promise is that `check` passing means the tracker is sound, and right now it doesn't.

Worth doing regardless of whether merge drivers ever ship — a hand-edit, a botched conflict
resolution, or a bad script can all produce the same state today.

## Implementation
Add the check to `validate` in `src/trck/scan.py`, alongside the existing structural checks.
It needs the raw row list, not `Graph.by_id`, precisely because the dict is what hides the
duplicate:

```python
seen = {}
for r in rows:
    if r.id in seen:
        errors.append(f"#{r.id} appears {rows.count(...)} times in index.jsonl "
                      f"(ids must be unique)")
    seen[r.id] = r
```
Emit **one** error per duplicated id, not one per extra row, and name the conflicting statuses
in the message — that is the detail that tells someone which side of a merge to keep.

**Decision: `load_index` dies.** Not a `validate` error. Three reasons:

1. The codebase's own doctrine, from `Issue.from_dict`'s docstring — the loader "enforces the
   structural/type contract and fails loud (no guessing, no recovery)". A duplicate id is that
   class of defect: the file is not a well-formed index.
2. The "let `check` report everything at once" argument does not survive contact — `load_index`
   already dies on malformed JSON and wrong-typed fields, so `check` cannot report past a
   structural problem anyway.
3. An errors-only approach still lets `trck done` rewrite a corrupt index, and worse, act on the
   wrong row: `resolve_ref` returns `exact[0]`, so a verb silently picks an arbitrary one of the
   duplicates.

The one cost — no verb can run until the file is hand-repaired — is acceptable, and hand-editing
`index.jsonl` is already the documented recovery for a botched merge.

**Implemented** as `check_unique_ids(rows)` in `src/trck/index.py`, called at the end of
`load_index`. It collects *every* duplicated id before dying, so one run reports the whole
problem rather than the first offender.

## Acceptance criteria
- [ ] Two index rows sharing an id make `trck check` exit nonzero with a clear error
- [ ] The message names the id and the conflicting statuses
- [ ] One error per duplicated id, not one per extra row
- [ ] Three-plus rows with the same id still report exactly one error
- [ ] A clean index is unaffected (no false positive on distinct ids)
- [ ] Decided and documented: does `load_index` refuse outright, or only `check` report?

## Notes
Blocks #ey2aruc — no union-merge strategy is trustworthy while its characteristic failure is
invisible to `check`.

Found while auditing whether #ey2aruc survived the flat-layout change (#2srvf6j). It did not,
and this gap is the reason.
