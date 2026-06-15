# Random alphanumeric ids — design

**Date:** 2026-06-15
**Status:** approved for implementation
**Issue:** #65 (epic #2; alternative to #64, which goes on-hold)

## Why

Ids are generated as `max(existing) + 1` (`trck:689`). That determinism is the
root cause of concurrent-creation collisions (#64): two branches that each run
`new` compute the *same* next integer, so their `index.jsonl` rows clash on
merge and one must be renumbered — which then breaks every `parent` /
`depends_on` that referenced the renumbered id.

Replacing `max+1` with a **short random alphanumeric id** breaks the
determinism: two branches draw different ids, their index rows union cleanly,
and because an id is never reassigned, cross-references never have to be
rewritten. Collision becomes improbable rather than structurally guaranteed; at
a tracker's real scale (thousands of issues) the birthday risk over a
~2.7×10¹⁰ id space is negligible. If this proves sufficient, #64 (the
renumber-on-merge driver) is YAGNI.

## Scope

In scope:

- `Issue.id` and the cross-reference fields (`parent`, `depends_on`) become
  **opaque strings**.
- A `gen_id` generator: 7 chars from an unambiguous base32 alphabet, with a
  within-branch collision guard.
- **Prefix resolution** for every place the CLI takes an id (git-short-hash
  style): exact id, then unique prefix, with ambiguity an error.
- Filenames key off the id (`<id>-slug.md`); listing order continues to come
  from `created`, never the id.
- A `legacy_id` field plus a **forced one-shot `renumber` command** that
  converts an existing integer-id base to random ids — rewriting `parent` /
  `depends_on`, renaming files, and recording each issue's prior integer id in
  `legacy_id`. `legacy_id` is a **resolvable alias** so old `#NN` references
  keep working.
- Backward-compatible loading of a not-yet-renumbered index (integer ids
  coerced to strings on read) and of legacy zero-padded filenames.
- Dogfooding: run `renumber` on trck's own `issues/`.
- Tests for all of the above.

Out of scope:

- #64's renumber-on-merge driver (on-hold; revisit only if a real collision is
  ever observed).
- Rewriting `#NN` prose mentions inside issue **bodies** during `renumber`
  (the `legacy_id` alias keeps those references resolvable via the CLI, so
  rewriting prose is unnecessary).
- Any change to sort/order semantics (already `created`-based where it matters).

## Design

### 1. Id type & alphabet

`Issue.id: int → str`. Cross-references follow: `parent: str | None`,
`depends_on: list[str]`. A new module constant:

```python
ID_ALPHABET = "23456789abcdefghjkmnpqrstuvwxyz"  # base32 minus 0 1 o l i (and uppercase, for typeability)
ID_LEN = 7                                        # 31**7 ≈ 2.75e10
ID_RE = re.compile(rf"^[{ID_ALPHABET}]+$")        # what gen_id produces
```

Lowercase-only keeps ids easy to type. The alphabet drops `0/o`, `1/l/i` to
avoid visual ambiguity. (Legacy numeric ids like `64` are *not* required to
match `ID_RE` — they are tolerated as opaque strings on read; only freshly
generated ids use this alphabet.)

### 2. Generation — `gen_id(ctx)`

Replaces `next_id`. Draws `ID_LEN` chars via `secrets.choice` (stdlib;
non-deterministic across processes, which is the point). **Within-branch
guard:** redraw if the candidate collides with any id already visible in the
index or on disk (the same union of sources `next_id` scans today). Only the
unseen cross-branch tail stays optimistic.

```python
def gen_id(ctx: Ctx) -> str:
    seen = _existing_ids(ctx)          # index rows ∪ filenames on disk
    while True:
        cand = "".join(secrets.choice(ID_ALPHABET) for _ in range(ID_LEN))
        if cand not in seen:
            return cand
```

`_existing_ids` factors out the index-∪-disk scan that `next_id` did inline.

### 3. Serialization & backward compatibility — `Issue.from_dict`

- `id` validation: must be a **non-empty string**. To load a not-yet-renumbered
  index, an integer `id` / `parent` / `depends_on[i]` is **coerced to `str`**
  on read (e.g. `64 → "64"`). This is what keeps the engine fully functional in
  the window between upgrading the engine and running `renumber`.
- New canonical field `legacy_id: int | None = None`, appended to `CANON_KEYS`
  (the `assert CANON_KEYS == [fields(Issue) …]` invariant is updated to match).
  Validated as int-or-None. Stripped from the canonical row when `None`
  (default), so non-migrated issues carry no `legacy_id` noise.

### 4. Filenames — `filename` / `FILENAME_RE` / scan ("normalize on touch")

- `filename(row)` drops zero-padding: `f"{row.id}-{row.slug}.md"`.
- `FILENAME_RE` widens its id group to the union of legacy-numeric and
  random forms (ids never contain `-`, so the first hyphen still splits
  id from slug):
  `^([0-9a-z]+)-([a-z0-9][a-z0-9-]*)\.md$`.
- The scanner normalizes a **legacy all-digit** id group by stripping leading
  zeros (`064 → 64`) so an old file maps to its coerced string id without an
  eager mass-rename. The canonical unpadded name is written whenever that issue
  is next touched (`mv` / `set`, which already rename), or proactively via
  `trck normalize`. (In practice the `renumber` dogfood renames everything in
  one pass; this tolerance covers the pre-`renumber` window and any consumer
  repo that delays.)

### 5. Display

Every `#{r.id:03d}` becomes `#{r.id}` (a small `fmt_id` helper, since the
pattern recurs across `validate`, the row renderers, `SUMMARY`, and the command
handlers). Legacy ids render as `#64`, not `#064`.

### 6. Reference resolution — `resolve_ref(rows, token)`

Drop `type=int` from every id argument (positional `id` on `mv`/`set`/`dep`/
`show`/`path`/…, plus `--parent`, `--depends`, `--add`, `--remove`). `get_row`
routes through `resolve_ref`, which resolves in **tiers** — within a tier,
exactly one match wins, more than one is an ambiguity error, zero falls through
to the next tier:

1. **Exact id** — `r.id == token`.
2. **Exact `legacy_id`** — `token` is all-digits and `int(token) == r.legacy_id`.
   (This is the alias that keeps `./trck show 65` working after renumber.)
3. **Unique id prefix** — `r.id.startswith(token)`.

A no-match token, or an ambiguous prefix (tier 3 with >1 hit), dies with a
message listing the candidate ids. Tier 2 before tier 3 means a numeric token
is read as the historical reference it almost always is; the policy is
documented in the resolver docstring and `--help`.

### 7. `renumber` command (forced one-shot)

New subparser `renumber` + handler `cmd_renumber`. Targets only **legacy
all-digit** ids (random ids are left untouched, so re-running is a no-op once no
numeric ids remain):

1. Load the index. For each legacy-numeric issue, `gen_id` a fresh id (guarding
   against both already-seen and just-assigned ids); build an `old → new` map.
2. Rewrite each issue's `parent` and every `depends_on` entry through the map;
   set `legacy_id` to the issue's prior integer id.
3. Rename each issue's file to the new `<id>-slug.md`.
4. Write the new index (via `finalize`, which also re-derives rollups, regenerates
   `SUMMARY.md`, and validates). Renames happen in place rather than through a
   temp-then-swap dance: the operation is a one-shot migration run inside an
   uncommitted git working tree, so a partial failure is fully recoverable with
   `git checkout -- issues/` — git is the rollback boundary, and `finalize`'s
   validate loudly reports any resulting inconsistency before the commit.
5. Print a summary (`N issues renumbered`) and a reminder that `#NN` prose
   mentions in bodies are unchanged but still resolve via the `legacy_id` alias.

Like the engine's other write verbs, the operation is invoked from the repo and
produces one reviewable commit.

### 8. Dogfooding

After the engine change lands and tests pass, run `./trck renumber` on this
repo's `issues/`, regenerate `SUMMARY.md`, confirm `./trck check` passes, and
commit the converted tracker **separately** from the engine change. Every
issue gains a `legacy_id`; `./trck show 65` continues to resolve via the alias.

## Testing

- **Generation:** `gen_id` output matches `ID_RE`, length `ID_LEN`; charset
  excludes the ambiguous characters.
- **Within-branch guard:** monkeypatch the RNG to emit a colliding id first,
  then a fresh one — assert `gen_id` skips the collision and returns the fresh
  id.
- **Resolution:** exact id; unique prefix; ambiguous prefix → error listing
  candidates; no match → error; `legacy_id` exact alias resolves; tier ordering
  (numeric token prefers `legacy_id` over a prefix hit).
- **Merge:** two independently-generated index rows union with no id clash and
  intact cross-references (simulates the two-branch scenario from #64).
- **Backward compatibility:** a not-yet-renumbered index with integer
  `id`/`parent`/`depends_on` loads (coerced to strings); a legacy `064-slug.md`
  filename scans to id `64`.
- **`renumber`:** rewrites `parent`/`depends_on` through the map, renames files,
  records `legacy_id`, is idempotent on a second run, and leaves pre-existing
  random ids untouched. Uses a throwaway temp tracker dir (never the real
  `issues/`).
- **`trck check`** passes on a renumbered tracker.
