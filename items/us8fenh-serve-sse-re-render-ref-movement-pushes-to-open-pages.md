# serve: SSE re-render — ref movement pushes to open pages

## Summary
Once the process knows the ref moved, open pages should show it without a manual reload.
Server-sent events over a held-open connection beat polling from the browser: one timer instead
of two, and it runs on a socket the server already holds.

## Acceptance criteria
- [ ] An `EventSource` endpoint holds one connection per open page and emits on ref movement.
- [ ] Both causes fire it: a write from this process, and a fast-forward from the poll loop.
- [ ] The page re-renders without losing selection, scroll position, or staged-but-unsent edits.
      `app.js` already has `keepingScroll` and `state.edits`; this must not trample either.
- [ ] A dropped connection reconnects, and a server that went away does not leave a page
      spinning forever.
- [ ] Held-open connections do not exhaust the serving layer's capacity.

## Notes
The event payload can be just the new SHA, with the page re-fetching the model. Simpler than
diffing server-side, and cheap over loopback.
