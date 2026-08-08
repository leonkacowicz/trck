# CI: skip the engine suites on issues- and docs-only pull requests

## Summary

A pull request that only edits tracker data or prose runs the whole of CI today: three
platforms of build/clippy/test, the conformance suite, the quality ratchet and the helper
scripts. None of that can be affected by a change under `issues/` or `docs/`. The one check
that *is* meaningful for such a change is `trck check`.

`paths-ignore` cannot express this. `main` requires three contexts — `quality ratchet`,
`rust (ubuntu-latest)` and `scripts` — and a workflow filtered out by paths never reports
them, so an issues-only pull request would sit "expected — waiting for status" forever. A job
**skipped by an `if:`** does report, and branch protection counts a skipped job as passing.
So the gate moves from the workflow trigger to the jobs.

## Acceptance criteria
- [ ] A `changes` job classifies the pull request's diff and outputs `code=true|false`.
- [ ] `scripts`, `quality` and `rust` run only when `code == 'true'`; they report as skipped
      otherwise, so the pull request stays mergeable.
- [ ] The classification is a tested helper, not shell buried in a workflow: an unreviewable
      path glob that silently widens is how a change stops being checked without anyone noticing.
- [ ] `trck check` still runs on every pull request — it moves out of the `rust` job into one
      that runs when `rust` is skipped, so the tracker is never unchecked.
- [ ] Pushes to `main` and `workflow_dispatch` always run everything.

## Notes

Docs-only counts as skippable too, on the same argument.

The classification is an allowlist, not a denylist: only `issues/**`, `docs/**` and
repository-root `*.md` are skippable, and anything else is code. `examples/`, `assets/`,
`conformance/` and `skills/` hold markdown that the engine and its specification actually
read, so a denylist keyed on `*.md` would skip the suite that checks them.

Fail-safe direction is *run everything*: an empty or unreadable diff classifies as code.

Follow-up outside the workflow file: `changed paths` and the new tracker job have to be added
to `main`'s required contexts. A required job whose dependency failed reports as skipped, and
skipped counts as success — so without that, a broken `changes` job would let a pull request
through with no CI at all.
