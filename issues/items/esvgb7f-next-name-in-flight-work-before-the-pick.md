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
- [ ] `trck next` prints an `in flight:` line naming non-terminal, non-`backlog` leaves before
      the recommended pick.
- [ ] The line is omitted entirely when nothing is in flight — no empty header.
- [ ] `trck ready` (the full list) is unchanged: the line belongs to the one-pick view.
- [ ] `next --json` carries the same information as a field rather than printing it, so the
      document stays a single ranked array plus its context.
- [ ] Scoping to a subtree (`ready ID --next`) scopes the in-flight line to that subtree too.
- [ ] A conformance fixture covers next-with-in-flight and next-with-nothing-in-flight.

## Notes
Leaves only, deliberately: an `ongoing` epic is ongoing because its children are, and listing it
would say nothing about who holds what. In this repo all four ongoing issues are epics, so the
line would be empty today — the fixture has to build the case rather than rely on the dogfood
tracker.
