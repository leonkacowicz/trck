# ready: narrow the actionable set to not-yet-started leaves

## Summary
The definition change itself, in both engines and in the spec.

`is_actionable` today excludes only `in-review` and `done`, so an unblocked `ongoing` leaf is
`ready`. Narrow it to exclude `ongoing` as well, making the predicate:

> **ready = `backlog` ∧ leaf ∧ no unmet (lifted) dependency**

`is_actionable` then reduces to `status == BACKLOG` for the fixed vocabulary. Keep the helper —
call sites read better for it, and it is the seam where a future status would slot in — but its
docstring has to stop justifying only the `in-review` case and state the rule the multi-actor
reading needs: a started issue is claimed, and `ready` hands out unclaimed work.

Nothing about blocking changes. An `ongoing` issue is still non-terminal, so it still blocks its
dependents and still contributes to the demand cone; it simply stops being proposed.

Depends on #esvgb7f so no release ships the narrowing without the in-flight line replacing what
it takes away.

## Acceptance criteria
- [x] `is_actionable` excludes `ongoing`, with a docstring giving the claim rationale.
- [x] `trck ready` / `trck next` omit unblocked `ongoing` leaves; `ready --json` and `next --json`
      likewise.
- [x] An `ongoing` issue still blocks its dependents and still counts toward demand ranking —
      covered by a test that would fail if the change leaked into `is_terminal` or the cone.
- [x] Conformance fixture: a tracker with an unblocked started leaf, asserting it appears in
      `list` and blocks its dependent, and is absent from `ready` and `next`.
- [x] `trck --help` / the `ready` subcommand help and `docs/` describe the narrowed rule.
- [~] Two-engine agreement — moot. There is one engine; `src/trck/config.py` and
      `--compare-bin` predate the port and are gone.

## Notes
This supersedes the reasoning in #6pvt7fy, which added the actionable flag back when each tracker
configured its own statuses. Worth a line in the release notes: it is a behaviour change a
downstream script could notice.

`is_actionable` and `is_in_flight` are now complements over the unfinished statuses, which a
test pins directly: every status is exactly one of actionable, in flight, or terminal. That is
the guard the reduction to `== BACKLOG` needs — a fifth status added to neither predicate would
otherwise go missing from both `ready` and the in-flight line.

`is_ready` lost its `!is_terminal` term as a consequence: `done` fails `is_actionable` on its
own now, and a redundant conjunct reads as though it were load-bearing.

The `ready` field in `src/html.rs` follows automatically — it calls `g.is_ready`. The
tree badge in `assets/app.js` is titled "nothing blocks this", which stays true; #vhzs9jx decides
what the HTML does with the badge once the board has a column.
