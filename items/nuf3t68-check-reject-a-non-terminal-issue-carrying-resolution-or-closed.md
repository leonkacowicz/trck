# check: reject a non-terminal issue carrying resolution or closed

## Summary
`(status, closed, resolution)` is a tuple the verbs maintain as a unit, but nothing enforces it.
A row with a non-terminal status **and** a `resolution` (or a `closed` timestamp) is a state no
verb can produce, and `trck check` accepts it silently.

The invariant, as the verbs implement it:

- `templates.py:184-186` — `move_issue` clears **both** `closed` and `resolution` on any move to
  a non-terminal status.
- `cmd_mutate.py:59-60` — `cmd_mv` refuses `--resolution` unless the target status is terminal.
- `cmd_set` cannot set `resolution` at all; it is reachable only through `mv`/`done`.

What `validate` actually checks (`scan.py:69`) is only that the resolution *value* is in the
configured vocabulary. Its **placement** — that a resolution implies a terminal status — is
unchecked, as is `closed` on a non-terminal row.

## Why it matters beyond hand-edits
This is the shape of corruption a field-wise merge of `index.jsonl` produces, and it gets there
**without either side's fields diverging** — so a "conflict when both sides touch the same field"
rule never fires:

```
base    status=done     closed=T1    resolution=None
ours    trck done #x --resolution wontfix   → status=done (unchanged), resolution=wontfix
theirs  trck mv #x ongoing                  → status=ongoing, closed=None, resolution=None

field-wise 3-way:  status     — only theirs changed → ongoing
                   resolution — only ours changed   → wontfix
                   closed     — only theirs changed → None

result  status=ongoing  closed=None  resolution=wontfix
```

See #ey2aruc, where this example is the reason the merge boundary has to be the whole lifecycle
tuple rather than individual fields. But the check is worth having regardless of whether merge
drivers ever ship: a hand-edit or a manually botched conflict resolution produces the same state
today, and `check` says `OK`.

## Implementation
Add to `validate` in `src/trck/scan.py`, in the per-row loop alongside the existing
`check_resolution` call. Both conditions key off `is_terminal(ctx.cfg, r.status)`:

- `resolution` set on a non-terminal row → error naming the status and the resolution.
- `closed` set on a non-terminal row → error.

Emit them as two distinct errors, since a merge can produce either independently. Errors, not
warnings: unlike the existing "terminal but depends on non-terminal" *warning*, this is not a
questionable-but-legal graph shape — it is a row the verbs cannot have written.

**Fixture note — the predicted breakage did not happen.** I expected hand-built `Issue` rows
(especially the timestamp back-compat tests) to trip the new check the way the layout change
shook out four fixtures. None did: the suite went from 718 to 725 tests with no adjustments.
The existing fixtures that set `closed` also set a terminal status, which is a small piece of
evidence that the invariant was already being respected everywhere the verbs are used.

Do **not** extend this to `pr`. A terminal issue keeping its pull-request link is desirable — it
is the review record for the change that resolved the issue, and `pr` is a forge-agnostic URL
that trck never interprets as open or merged.

## Acceptance criteria
- [ ] A non-terminal row carrying a `resolution` makes `trck check` exit nonzero
- [ ] A non-terminal row carrying `closed` makes `trck check` exit nonzero
- [ ] Both firing on one row produce two distinct errors
- [ ] Terminal rows with `closed` and/or `resolution` are unaffected (no false positive)
- [ ] A terminal row carrying a `pr` stays valid — explicitly tested, so nobody adds that check later
- [ ] Existing fixtures that construct rows directly still pass

## Notes
Same class as #s5585hq: an invariant the verbs uphold that `check` does not verify, so anything
writing `index.jsonl` from outside the verbs can violate it silently.
