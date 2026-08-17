# serve: the HTTP layer — loopback listener, compiled-in assets, clean shutdown

## Summary
The plumbing `serve` needs before it can do anything interesting: bind a listener, answer
requests, stop cleanly. No tracker writes here — this child ends with the same page `trck html`
already emits, served from a live process instead of written to a file.

The build-or-borrow decision lands here. std has no HTTP server, so this is either hand-rolled
HTTP/1.1 over `TcpListener` plus a thread pool, or a crate. **The dependency rule permits the
crate** — the constraint is one self-contained binary, and anything that links statically
qualifies — so decide it on build cost and supply-chain surface. Record the reasoning in this
issue either way: it is the binary's first listening socket, and its first dependency decision
under the new rule.

## Acceptance criteria
- [x] `trck serve [--port N]` binds **127.0.0.1 only**, and the help text says so rather than
      leaving it implied.
- [x] `GET /` serves what `render_html` produces; `app.css` and `app.js` come from the
      compiled-in copies, never from disk.
- [x] A port already in use fails with a diagnostic naming the port. The `unwrap`/`expect`/
      `panic` lints apply here as everywhere — a busy port is not a stack trace.
- [x] Ctrl-C shuts down without leaving the port bound.
- [x] Conformance covers the refusal cases (bad `--port`, no tracker resolvable); request
      parsing and routing are unit-tested.

## Notes
`cmd_html` already assembles the page from `build_model` plus the compiled-in assets. This
reuses that path rather than forking it.

Reads resolve from the ref via `git show`, so there is no working tree to guard and no lock to
hold; concurrency is whatever the chosen serving layer gives.

### The decision: hand-written, and why (PR #67)
No crate. Decided on cost rather than on a prohibition, as the rule now requires:

- **What is actually needed is small.** One method, three fixed paths, no query parsing, no
  chunked bodies, no TLS, no keep-alive, no content negotiation — and every response body is
  already in memory before the head is written. That is `src/serve/http.rs`, a few dozen lines,
  against a tree of crates to audit and cross-build for all six release targets.
- **Narrow is a security property here, not a limitation.** The parser accepts what a browser
  on loopback sends and refuses the rest, which is less surface than a general server, not more.
- **The later children do not change the answer.** SSE (#us8fenh) wants a connection held open
  on a socket this already owns, and POST-to-`Op` (#mcmfmca) wants a `Content-Length` body —
  both are additions to this file, not reasons to have taken a framework.

Revisit only if something here needs TLS, HTTP/2, or a websocket. Nothing does, and a tracker
served on 127.0.0.1 should not.

### Two things worth stating rather than rediscovering
**Loopback is not the whole of the answer; the `Host` header is the other half.** Binding
127.0.0.1 stops another *machine*. It does nothing about another *site* whose hostname resolves
to 127.0.0.1 and whose page can have the visitor's own browser fetch this one and read the
tracker out of the response. A request naming a host that is not this machine's own is refused,
before the method is even looked at. This was not in the criteria above; it costs one header the
parser already has in hand, and it becomes load-bearing the moment #mcmfmca makes a POST here a
write to a shared branch.

**Shutdown is the default `SIGINT` disposition, on purpose.** Nothing installs a handler — that
needs a raw `signal()` call and `unsafe` is forbidden in this crate — so the kernel terminates
the process and closes the listener with it. It would also have nothing to do: no file written,
no lock held, no temp tree left. `tests/serve.rs` asserts the outcome rather than the mechanism,
by binding the port again after the signal.

### What this child left for its siblings
- The tracker is read per request, not cached. Correct by construction and it costs what
  `trck html` costs (~0.7s for this repo's own tracker off the ref). Caching keyed on the ref
  SHA belongs with the watcher that would invalidate it — #eemua4s.
- The page is still the self-contained one, so it inlines both assets and never fetches
  `/app.css` or `/app.js`. Those routes exist so that nothing in a live process can ever answer
  an asset request out of a working tree that may be on another branch.

### The conformance suite needed a timeout
`serve` is the first verb whose job is to run until it is signalled, and `run.py` had no limit —
a fixture that started a server would hang the suite and CI with it, with no output to read. It
now kills an invocation after 30s and reports status 124, which no verb produces. So a fixture
may assert `serve`'s refusals, which exit before the socket exists, and never a running one.
