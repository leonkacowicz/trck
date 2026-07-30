# add the migrate-layout verb

## Summary
The one-shot, idempotent relocation of a legacy tracker into `items/`. Registered as
**`trck repo migrate-layout`** — see #qs4zwzr, which this depends on so the verb is born in its
final namespace rather than renamed a release later.

Deliberately conservative about the one ambiguity a legacy tracker can carry: if a file's folder
disagrees with its index status, the two sources of truth have already drifted and only the
author knows which side is right, so it stops rather than silently canonizing one.

## Implementation

**`src/trck/cmd_maint.py`** — add `cmd_migrate_layout`:
```python
def cmd_migrate_layout(args) -> None:
    ctx = build_ctx_or_die(args, guard_layout=False)
    stale = detect_legacy_layout(ctx.cfg, ctx.dir)
    if not stale:
        print(f"migrate-layout: nothing to migrate (already flat in {ITEMS_DIR}/)")
        return

    rows = load_index(ctx)
    by_id = {r.id: r for r in rows}
    dest_dir = ctx.dir / ITEMS_DIR

    drift, collisions, moves = [], [], []
    for p in stale:
        m = FILENAME_RE.match(p.name)
        iid = file_id(m)
        row = by_id.get(iid)
        if row is not None and row.status != p.parent.name:
            drift.append(f"#{iid}: index says '{row.status}', file sits in "
                         f"'{p.parent.name}/'")
            continue
        dest = dest_dir / p.name
        if dest.exists():
            collisions.append(f"#{iid}: {dest} already exists")
            continue
        moves.append((p, dest))

    if drift:
        detail = "\n  ".join(drift)
        die(f"index status and folder disagree for {len(drift)} issue(s); fix the "
            f"index (or move the files) so they agree, then re-run:\n  {detail}")
    if collisions:
        detail = "\n  ".join(collisions)
        die(f"destination already occupied for {len(collisions)} file(s):\n  {detail}")

    if getattr(args, "dry_run", False):
        print(f"migrate-layout: would move {len(moves)} file(s) into {ITEMS_DIR}/")
        for src, dest in moves:
            print(f"  {src.parent.name}/{src.name} -> {ITEMS_DIR}/{dest.name}")
        return

    dest_dir.mkdir(parents=True, exist_ok=True)
    for src, dest in moves:
        shutil.move(str(src), str(dest))

    # Drop the status folders that are now empty. A folder holding anything else
    # (a README, a scratch note) is left alone — rmdir refuses a non-empty dir.
    for folder in {src.parent for src, _ in moves}:
        try:
            folder.rmdir()
        except OSError:
            pass

    finalize(ctx, rows)  # rewrite SUMMARY.md with items/ links, then validate
    print(f"migrate-layout: moved {len(moves)} file(s) into {ITEMS_DIR}/")
```
`cmd_maint.py` already imports `shutil`, `finalize`, `load_index`, `build_ctx_or_die`; add
`FILENAME_RE`, `ITEMS_DIR`, `file_id`, and `detect_legacy_layout`.

**`src/trck/cli.py`** — register under the `repo` group from #qs4zwzr:
```python
    ml = rsub.add_parser("migrate-layout",
                         help="one-shot: move issue files from status folders into items/",
                         description="One-shot migration to the flat layout: move every "
                                     "issue body out of its per-status folder into "
                                     "items/, so status lives only in index.jsonl. "
                                     "Idempotent; stops without writing if a file's "
                                     "folder disagrees with its index status.")
    ml.add_argument("--dry-run", action="store_true",
                    help="print what would move, write nothing")
    ml.set_defaults(func=cmd_migrate_layout)
```

**Tests** — new `tests/test_layout.py::TestMigrateLayout`: every file lands in `items/`; status
is preserved from the index; `check` passes afterward; a second run reports "nothing to migrate";
`--dry-run` writes nothing; it dies on folder/index status disagreement (leaving files in place);
it dies on a destination collision; non-issue files keep their folder alive.

## Acceptance criteria
- [ ] `trck repo migrate-layout` moves every issue body into `items/` and removes the emptied folders
- [ ] Status values in `index.jsonl` are untouched by the migration
- [ ] `trck check` passes on the migrated tracker
- [ ] Idempotent — a second run writes nothing and says so
- [ ] `--dry-run` prints the planned moves and writes nothing
- [ ] Dies without writing when a file's folder disagrees with its index status
- [ ] Dies without writing on a destination collision
- [ ] A folder holding non-issue files survives the migration
- [ ] Full suite green

## Notes
Depends on #qs4zwzr so the verb is registered under `repo` from birth. If that decision slips,
this can land at the root and be moved later — at the cost of a rename in a released CLI.

Step-by-step TDD cycle with the full test code: `docs/plans/2026-07-30-flat-items-layout.md`
(Task 4).
