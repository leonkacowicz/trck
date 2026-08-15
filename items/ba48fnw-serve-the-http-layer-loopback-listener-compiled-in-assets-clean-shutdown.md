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
- [ ] `trck serve [--port N]` binds **127.0.0.1 only**, and the help text says so rather than
      leaving it implied.
- [ ] `GET /` serves what `render_html` produces; `app.css` and `app.js` come from the
      compiled-in copies, never from disk.
- [ ] A port already in use fails with a diagnostic naming the port. The `unwrap`/`expect`/
      `panic` lints apply here as everywhere — a busy port is not a stack trace.
- [ ] Ctrl-C shuts down without leaving the port bound.
- [ ] Conformance covers the refusal cases (bad `--port`, no tracker resolvable); request
      parsing and routing are unit-tested.

## Notes
`cmd_html` already assembles the page from `build_model` plus the compiled-in assets. This
reuses that path rather than forking it.

Reads resolve from the ref via `git show`, so there is no working tree to guard and no lock to
hold; concurrency is whatever the chosen serving layer gives.
