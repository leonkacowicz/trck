# extract the pure status transition out of move_issue

## Summary
`move_issue` does two unrelated things: it applies a status transition (validate against the
vocabulary, set the status, stamp `started`/`closed` from the roles), and it guards that the
issue's body file exists. The transition is pure; the guard is filesystem contact. Splitting
them lets each caller take what it actually needs.

**Why the coupling is a problem.** `normalize_statuses` routes through `move_issue` purely to
get the date stamping — it derives a parent's status from its children and needs the same
stamping an explicit move would produce. But it inherits the file guard as a side effect, which
makes it unusable anywhere the working tree is not settled:

- The merge driver (#ex5cugg) deliberately does **not** normalize, because git gives no ordering
  guarantee between writing `index.jsonl` and checking out an `items/*.md` the other side added.
  A merge introducing a new issue could hit the guard and abort — the driver dying on a file git
  simply had not written yet.
- The consequence is user-visible: after a merge that shifts child statuses, a parent's rollup
  can be stale and `trck check` complains until someone runs `trck repo normalize`.

**The guard itself is correct and should stay** for the interactive verbs. `trck done` on an
issue whose body is missing should fail immediately rather than surface later as a `check`
error. It only became reachable in #zk5k59n — before the flat layout it sat inside a
`if old != new` branch that could never fire once the path stopped encoding status.

## Implementation

Split `src/trck/templates.py`:

```python
def apply_status(cfg: dict, row: Issue, new_status: str) -> None:
    """Validate against the vocabulary, set the status, stamp the dates the roles
    imply. Pure — no filesystem contact — so it is safe wherever the working tree
    may not be settled (merge drivers, dry runs, in-memory normalisation)."""

def move_issue(ctx: Ctx, row: Issue, new_status: str) -> None:
    """apply_status plus the guard that the body file exists. The interactive verbs
    use this: acting on a specific issue whose body has gone missing should fail
    loudly, here, not later as a `check` error."""
```

Then point `normalize_statuses` (`graph.py`) at `apply_status`. It is deriving a value rather
than carrying out an instruction about a specific issue, so the guard was never what it wanted.

**Also fix the stale docstring** on `normalize_statuses`, which still claims it "relocates
changed parents through `move_issue` (file move + date stamping)". There has been no file move
since #v7zzefd.

Leave the driver's own behaviour alone in this issue — enabling normalisation inside
`merge-index` is a separate change with its own risk, and is only *possible* once this lands.

## Acceptance criteria
- [ ] `apply_status(cfg, row, new_status)` exists, does no filesystem I/O, and stamps
      `started`/`closed`/`resolution` exactly as today
- [ ] `move_issue` keeps the missing-body guard and produces identical behaviour to today for
      every interactive verb
- [ ] `normalize_statuses` no longer touches the filesystem — a tracker whose body files are all
      absent can still be normalised in memory
- [ ] The unknown-status error still fires from both entry points
- [ ] `normalize_statuses`'s docstring no longer claims a file move
- [ ] Full suite green; no behaviour change for any existing verb

## Notes
Found while closing #ey2aruc: the merge driver had to skip normalisation, and the reason turned
out to be one guard line rather than anything inherent. Worth undoing on its own terms — a pure
derivation should not depend on the working tree — with the driver benefit as a consequence.
