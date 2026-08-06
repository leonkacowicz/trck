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
- [ ] `is_actionable` excludes `ongoing`, in `src/trck/config.py` and `src/config.rs`,
      with a docstring giving the claim rationale in both.
- [ ] `trck ready` / `trck next` omit unblocked `ongoing` leaves; `ready --json` and `next --json`
      likewise.
- [ ] An `ongoing` issue still blocks its dependents and still counts toward demand ranking —
      covered by a test that would fail if the change leaked into `is_terminal` or the cone.
- [ ] Conformance fixture: a tracker with an unblocked started leaf, asserting it appears in
      `list` and blocks its dependent, and is absent from `ready` and `next`.
- [ ] `python3 conformance/run.py --compare-bin target/release/trck` agrees across both engines.
- [ ] `trck --help` / the `ready` subcommand help and `docs/` describe the narrowed rule.

## Notes
This supersedes the reasoning in #6pvt7fy, which added the actionable flag back when each tracker
configured its own statuses. Worth a line in the release notes: it is a behaviour change a
downstream script could notice.

The `ready` field in `src/html.rs` follows automatically — it calls `g.is_ready`. The
tree badge in `assets/app.js` is titled "nothing blocks this", which stays true; #vhzs9jx decides
what the HTML does with the badge once the board has a column.
