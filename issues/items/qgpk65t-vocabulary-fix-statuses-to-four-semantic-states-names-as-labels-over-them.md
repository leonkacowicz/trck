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
- [x] Resolutions fixed to a canonical set, valid only on `done`.
- [ ] The 58 sites reading the config vocabulary collapse onto state predicates.
- [ ] Transition validity is expressible; the guards themselves stay with their own issues.
- [ ] `index.jsonl` stores the canonical state and, where a tracker uses one, its label.

## Notes
Reshapes `s3d6xyz` (rename/reorder statuses): renaming survives as a display alias, reordering
does not survive at all. Decide its fate when this lands.

## Decisions

Settled while building this, in the order they came up.

**`review` is not `waiting`.** The rule that separates it from `depends_on`: use `depends_on`
when the blocker is real work someone will do and close; use `review` when making it a task
would be inventing one. A code review forces it — the reviewer is judging your deliverable, not
producing one, so a task per reviewable issue would be a fiction and would double the tracker.
The same holds for a vendor reply or a sign-off nobody here will close.

**`ready` stays derived and is not a fifth state.** It is orthogonal to the state axis, not a
member of it — a `todo` issue may be ready or blocked, and so may a `doing` one — so making it a
state would break the partition. It is also a property of the graph: closing X changes the
readiness of issues nobody edited, which as stored state is a cascade write on every close and
silent corruption on every missed one. And it is leaf-only, which is structural. `index.jsonl`
holds declared facts; readiness is derived, like progress and demand.

**`doing` + blocked is legal but anomalous.** It arises without touching the issue at all — a
dependency discovered mid-flight, a blocker reopened, a parent gaining one its children inherit —
so it cannot be forbidden without firing the guard on the wrong issue. It is already handled
quietly (`is_ready` excludes it, `list` shows `needs #X`); what it deserves is a `check` warning.
That is the same shape as `tfhhp8h`: the declared status and the derived graph disagreeing. One
coherence check likely covers both. The principle: states are declared, blocking is derived, and
derived facts may contradict declared ones — the engine surfaces the contradiction rather than
making it unrepresentable.

**One status per state, no exemption.** An earlier draft let several statuses mean `review` so a
tracker could name `qa` and `awaiting-deploy` apart. Custom fields already give one value per
key, already filter, and under `eemqu4g`'s schema can declare their allowed values — and however
history lands (`gybeetp`), a field change is as visible as a status change. Nothing was left that
only a status could do, so the weaker of two overlapping vocabularies goes. This promotes
`eemqu4g`: it now carries every distinction beyond the four states, so it needs declared allowed
values and a declared applicable state.

**The test for whether an extension point is worth keeping:** does it stay local, or does it
reshape the core? Renaming the four states is local. Arbitrary priorities deform the demand
vector. Extra review statuses were local *and* redundant, which is why they went for a different
reason than priorities did.

## Landed / open

Landed in `88d642d` and `52cde28`: the four states, `state_of`, the predicates collapsed onto
them, `check_status_states`, the `state -> name` table form, and `init` scaffolding it. Fully
backward compatible — `role` and `actionable` still derive, so this repo's tracker was untouched.

Still open here:
- [ ] `pr` renamed to `review_url`. A persisted-field rename, so it needs a migration path;
      `Issue.from_dict` already has precedent, migrating `milestone` to a label.
- [x] Resolutions fixed to a canonical set, valid only on `done`. Three of them —
      `superseded`/`wontfix`/`duplicate` — and the *absence* of one is the load-bearing
      case: `select_shipped` keys off it, so a resolution means closed-without-shipping
      and there is deliberately no `fixed`. `trck.json` is now empty of vocabulary.
- [ ] Transition validity, including the `doing`-and-blocked warning above.
- [ ] `index.jsonl` stores the state, with the status name as a display alias. Deferred to
      `rbast9r`, which is where the rows get rewritten — until then a rename would be caught by
      `validate` (the stored status is no longer a configured name) rather than silently break.
