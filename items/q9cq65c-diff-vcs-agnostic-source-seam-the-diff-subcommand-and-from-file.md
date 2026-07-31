# diff: VCS-agnostic source seam — the diff subcommand and --from FILE|-

## Summary
`trck diff` must not require git. Every query and mutate verb today is VCS-free — only
`install-hook` and `setup-git` shell out (`cmd_maint.py:95,549,579`), and both are explicitly
git-flavoured and already `die("not a git repository")`. `diff` would be the first *reading* verb to
need history, so the dependency gets isolated on purpose rather than by accident.

This issue owns the `diff` subcommand and the **snapshot** abstraction it reads through. Nothing
downstream of the seam knows where a snapshot came from; git (#wtmfdhr) is one provider among
several, added on top.

A snapshot is:
- `rows` — the parsed index rows,
- `body(id)` — the issue's markdown body, or **`None` meaning "unavailable from this source"**,
  which must stay distinguishable from `""` meaning "empty body" (#6xcseef depends on the
  difference: unavailable is not the same as unchanged),
- `label` — a short human string for the output header (`HEAD`, `main`, `old-index.jsonl`, `stdin`).

Sources this issue provides:
- the **working tree** (the default for the new side),
- `--from PATH` where PATH is an `index.jsonl`, or a whole tracker dir (which also yields bodies),
- `--from -` — read an `index.jsonl` from stdin; rows only, no bodies.

`--to` mirrors `--from` and defaults to the working tree, so `trck diff` is symmetric and every
combination is expressible without git.

## Acceptance criteria
- [ ] `trck diff --from <index.jsonl>` works in a directory that is not a git repository, with git
      absent from `PATH`.
- [ ] `git show main:issues/index.jsonl | trck diff --from -` produces the same result as the git
      provider would for the same revision.
- [ ] `--from <tracker-dir>` yields bodies; `--from <index.jsonl>` and `--from -` return `None` from
      `body()` rather than pretending bodies are empty.
- [ ] `--to` accepts the same forms and defaults to the working tree.
- [ ] The change model (#u8qaqwr) and every renderer consume snapshots only — no module below this
      seam imports `subprocess` or mentions a revision.
- [ ] Tests build snapshots from fixture files, with no git repository or fixture commits involved.
- [ ] A malformed or unreadable source fails with a clear message naming the path, not a traceback.

## Notes
- The payoff beyond VCS-neutrality is testability: every layer above can be tested from plain
  fixture files, so only #wtmfdhr needs a real git fixture.
- Also makes `diff` usable under jj, hg, or on an unpacked tarball / backup copy.
- Deliberately **not** doing: a trck-maintained mutation journal as a third source. It would be a
  second source of truth to keep consistent, would conflict on merge, and would badly re-implement
  what git already does. `index.jsonl` is state; the VCS is history.
- Also deliberately not doing: reading git objects with stdlib `zlib`. Packfile parsing is a
  mountain of code for a single-file stdlib-only tool, and the payoff is nil — git is present
  wherever the repo is.
