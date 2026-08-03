# vocabulary: fix statuses to four semantic states, names as labels over them

## Summary
The semantics already exist — `role` (`initial`/`active`/`terminal`) plus `actionable` — bolted
onto a free-form label list. Invert it: make the state primary and the name a label over it.

The current config already tells us the state set:

| state | today |
|---|---|
| `todo` | `role: initial` |
| `doing` | `role: active` |
| `waiting` | active but `actionable: false` (in-review) |
| `done` | `role: terminal`, carries a resolution |

Two things follow that are not available today:

- **`waiting` should carry what it waits on**, not just a name. `in-review` already carries a
  PR; generalising that is more semantic than any label can be.
- **Transitions become checkable.** `mv` allows anything → anything right now. Two open issues —
  `tfhhp8h` (closing with unfinished dependencies) and `wh3mv52` (closing a parent with open
  descendants) — are state-machine rules, awkward to bolt on and natural once the states are
  fixed.

Resolutions are folded in here rather than split out: a resolution only exists on `done`, so the
two are one model.

## Acceptance criteria
- [ ] Four states in code. A tracker may name them and may map several names onto one state,
      but may not invent a state.
- [ ] `waiting` carries a reason — the PR link generalised.
- [ ] Resolutions fixed to a canonical set, valid only on `done`.
- [ ] The 58 sites reading the config vocabulary collapse onto state predicates.
- [ ] Transition validity is expressible; the guards themselves stay with their own issues.
- [ ] `index.jsonl` stores the canonical state and, where a tracker uses one, its label.

## Notes
Reshapes `s3d6xyz` (rename/reorder statuses): renaming survives as a display alias, reordering
does not survive at all. Decide its fate when this lands.
