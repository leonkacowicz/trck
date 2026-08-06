# CI: a consistency check a consumer repo can drop in

## Summary
The pre-commit hook is a **convenience, not a gate** — anyone can pass `--no-verify`, and a
tracker only has to be inconsistent once, on one machine, for the damage to be committed. The
place a consistency check actually binds is CI, where the commit is already public and the
check is not the committer's to skip.

This repo has had that all along: a `trck check` step in its own workflow. Every repo that
adopts trck has to reinvent it, from a `trck` they must first install on a runner. That is
the gap.

What "drop in" could mean, roughly in order of how much this project takes on:

- **A documented snippet.** A dozen lines of YAML in the README and the scaffolded
  `issues/README.md`: install the binary, run `trck check`. Costs nothing, ages badly — every
  consumer copy freezes whatever the install line looked like that day.
- **Scaffolded by a verb.** `trck repo install-ci` (a sibling of `install-hook`) writes
  `.github/workflows/trck.yml`, and `trck init --ci` asks for it. Same shape as the hook, so
  the mental model carries over, and the engine owns the content.
- **A published action.** `leonkacowicz/trck-action@v1` — a consumer writes three lines and
  never sees the install step. Most convenient, and the largest ongoing commitment: an action
  is a second release artifact with its own versioning.

Note what this is not: a re-run of `conformance/`. It answers "is *this repository's tracker*
internally consistent" — no dangling parents, no index row without a body, no cycles — which
is exactly `trck check`, run somewhere the answer cannot be ignored.

## Acceptance criteria
- [ ] One of the shapes above chosen, with the reason recorded here.
- [ ] Whatever ships is verified against a real repository that is not this one — a scaffolded
      workflow nobody has watched fail on a genuinely broken tracker is a workflow that has
      not been tested.
- [ ] The check fails the build on an inconsistent tracker, and says which issue is at fault
      rather than only that something is wrong.
- [ ] Pinned by version, not floating: a consumer's CI should not change behaviour because a
      new trck was released overnight.

## Notes
Raised while checking whether the installed pre-commit hook still works once vendoring is gone
(it does — it falls back to `trck` on `PATH`). It silently does nothing when no engine is
found at all, which is fine for a convenience and is exactly why the real check belongs here.

Related: `#djx63gk`. This repo's own `trck check` currently lives in the Python `test:` job of
`.github/workflows/ci.yml` — the job the cutover deletes — so the cutover has to carry it over
or lose it silently.
