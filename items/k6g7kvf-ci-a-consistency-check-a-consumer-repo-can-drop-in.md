# CI: a consistency check a consumer repo can drop in

## Summary
The pre-commit hook is a **convenience, not a gate** — anyone can pass `--no-verify`, and a
tracker only has to be inconsistent once, on one machine, for the damage to be committed. The
place a consistency check actually binds is CI, where the commit is already public and the
check is not the committer's to skip.

This repo has had that all along: a `trck check` step in its own workflow. Every repo that
adopts trck has to reinvent it, from a `trck` they must first install on a runner. That is
the gap.

**Decided: a published action, plus documentation. No verb.**

A scaffolding verb was considered and rejected. `install-hook` earns its place because a hook
is per-clone and git will not share it — there is no way to get one except to write it locally.
A workflow file is the opposite: it is committed once and shared with everyone who clones, so
scaffolding it saves a single copy-paste and in exchange puts YAML generation inside the
engine, where it has to be versioned, tested and kept current with whatever GitHub changes
next. The engine has no business knowing what a runner looks like.

An action carries that knowledge instead, in the one place it belongs, and a consumer writes
three lines that do not mention installing anything. The documentation covers everyone who is
not on GitHub Actions.

Note what this is not: a re-run of `conformance/`. It answers "is *this repository's tracker*
internally consistent" — no dangling parents, no index row without a body, no cycles — which
is exactly `trck check`, run somewhere the answer cannot be ignored.

## Acceptance criteria
- [x] Shape chosen: a published action plus documentation, no verb. Reason above.
- [ ] The action installs a **pinned** trck and runs `check`. A consumer's CI must not change
      behaviour because a release happened overnight, so the version is an input with a
      default, not "latest".
- [ ] It fails the build on an inconsistent tracker and names the issue at fault, rather than
      only reporting that something is wrong.
- [ ] It finds the tracker the way every verb does — by discovery — with an input to override,
      so a repo whose tracker is not at `issues/` is not excluded.
- [ ] Verified against a real repository that is not this one, watched failing on a genuinely
      broken tracker. An action nobody has seen go red has not been tested.
- [ ] Documented for everyone not on GitHub Actions: what to install and what to run, so the
      action is a convenience over a documented command rather than the only path.
- [ ] Decide where it lives — its own repository, as actions conventionally do, or a
      subdirectory here — and how it is versioned against the engine's releases.

## Notes
Raised while checking whether the installed pre-commit hook still works once vendoring is gone
(it does — it falls back to `trck` on `PATH`). It silently does nothing when no engine is
found at all, which is fine for a convenience and is exactly why the real check belongs here.

Related: `#djx63gk`. This repo's own `trck check` currently lives in the Python `test:` job of
`.github/workflows/ci.yml` — the job the cutover deletes — so the cutover has to carry it over
or lose it silently.
