# next: name in-flight work before the pick

## Summary
`trck next` answers "what should I do" with exactly one row and no context. Add a line above it
naming what is currently in flight — the non-terminal, non-`backlog` leaves — so the reader sees
what is already taken:

```
in flight: #fkrp9dh #u5fc5vm
→ #c37tmn5 backlog  medium  completion: value providers for ids, statuses, …
```

This is useful on its own — an idle picker learns what their colleagues hold without having to
run a second command — and it is what makes #gccs68j safe to ship. Once `ready` stops offering
started work, this is where a returning solo user finds their own in-progress task again, and it
no longer competes with fresh work for the top slot.

Ships **before** the narrowing, so no release drops the reminder.

## Acceptance criteria
- [x] `trck next` prints an `in flight:` line naming non-terminal, non-`backlog` leaves before
      the recommended pick.
- [x] The line is omitted entirely when nothing is in flight — no empty header.
- [x] `trck ready` (the full list) is unchanged: the line belongs to the one-pick view.
- [x] `next --json` carries the same information as a field rather than printing it, so the
      document stays a single ranked array plus its context.
- [x] Scoping to a subtree (`ready ID --next`) scopes the in-flight line to that subtree too.
- [x] A conformance fixture covers next-with-in-flight and next-with-nothing-in-flight.

## Notes
Leaves only, deliberately: an `ongoing` epic is ongoing because its children are, and listing it
would say nothing about who holds what. In this repo all four ongoing issues are epics, so the
line would be empty today — the fixture has to build the case rather than rely on the dogfood
tracker.

**The JSON shape, as decided.** "A field rather than printing it" cannot be an array's field,
so `--json` became an object: `{"in_flight": [...], "ready": [...]}`, in-flight as whole rows
rather than bare ids. Two calls that could have gone the other way:

- **Both verbs, not just `next`.** `ready --json` and `next --json` had one shape, asserted by
  a fixture; giving only `next` the object would have split them for the sake of a key. The
  cost is the one asymmetry left: `ready --json` carries `in_flight` where the human `ready`
  prints no line. A caller that does not want the context ignores a key, and one that does
  cannot invent it — whereas the human full list already renders every row the line would name,
  which is why it stays out of *that* view.
- **Whole rows, not ids.** Every other `--json` document emits issue objects; ids would have
  made this the one place a consumer needs a second call to render what it was handed.

Breaking: `ready`/`next --json` no longer parse as an array. Both are pre-1.0 and the fixtures
that pinned the array shape moved with the change.
