# serve: POST to Op — the page's staged edits become in-process write verbs

## Summary
`app.js` already stages edits and computes the exact commands: `state.edits`, `setEdit`,
`commandFor(id, field, value)`, `pendingCommands()`, `renderPending()`. Today that panel's whole
job is to be copied into a terminal. This child closes the loop — the same staged edits POST to
the server, which builds the corresponding `Op` and applies it through the git backend.

The seam already exists: `finalize` returns a `Changeset` and an `Op` (#yuj6azz), and a backend
applies it. `serve` is a third caller of the path the CLI already uses. It must not shell out to
itself.

## Acceptance criteria
- [ ] A POST carrying staged edits produces one `Op` per edit and applies them through the same
      backend the CLI uses — no subprocess, no round trip through a rendered command string.
- [ ] The `Op` built for an edit is the one `commandFor` renders for it, so the pending panel
      keeps telling the truth about what will happen.
- [ ] A validation failure returns the engine's own diagnostic to the page and changes nothing.
- [ ] A rejected push surfaces to the page rather than being swallowed. Displaying the resulting
      pending state is a separate child.
- [ ] The response carries the new ref SHA, so a page can tell its own write from someone else's.
- [ ] Tests cover a status move, a `set`, a `dep`, a validation failure, and a rejected write.

## Notes
Body edits are out of scope here — they go through `trck edit`'s path (#zxz9vu2).

Open: one request per edit, or one request for the staged batch. A batch is one commit and one
push, which matches how the panel already groups them.
