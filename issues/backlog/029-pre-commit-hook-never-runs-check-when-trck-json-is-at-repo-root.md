# pre-commit hook never runs check when trck.json is at repo root

## Summary
`cmd_install_hook` (in `trck`, ~lines 1186–1218) generates a pre-commit hook that
guards `trck check` behind a grep over staged paths. When the tracker dir is the git
repo root (i.e. `trck.json` sits at the repo root, e.g. a repo set up with
`trck init .`), the generated guard never matches any staged file, so the hook
**silently never runs `trck check`** — the consistency check it exists to enforce is
defeated.

## Root cause
`rel` is computed as the tracker dir relative to the repo root:

```python
rel = ctx.dir.resolve().relative_to(root.resolve()).as_posix()
```

When the tracker dir **is** the repo root, `Path.relative_to(self)` returns
`Path('.')`, so `rel == "."`. The grep pattern is then built from
`rel_re = rel.replace(".", r"\.")` → `"\\."`, producing:

```bash
grep -qE '(^|/)\./'
```

`git diff --cached --name-only` emits repo-root-relative paths like `index.jsonl` or
`issues/foo.md` — none contain the literal `./`, so the guard never matches and the
`trck check` branch is never entered. (The `"$root/{rel}/trck"` → `"$root/./trck"`
paths themselves are harmless; only the guard is broken.)

## Reproduce
1. `git init demo && cd demo`
2. `trck init .` (so `trck.json` lands at the repo root) and install the hook.
3. Stage a tracker change and commit.
4. Observe `trck check` is never invoked (verify by staging a deliberately
   inconsistent index — the commit still succeeds).

## Acceptance criteria
- [ ] When the tracker dir is the repo root (`rel == "."`), the pre-commit hook runs
      `trck check` on every commit that stages any tracked file (the whole repo is the
      tracker, so the guard should effectively always fire).
- [ ] The existing nested-tracker case (e.g. `issues/`) still only fires when files
      under the tracker dir are staged.
- [ ] A regression test covers hook-body generation for the root-tracker case and
      asserts the guard is not the never-matching `(^|/)\./` pattern.

## Notes
- Code: `trck` `cmd_install_hook`, lines ~1196 (`rel = …`) and ~1202–1214 (guard
  construction).
- Possible fix: special-case `rel == "."` — drop the path-prefix guard entirely (run
  `check` on any staged change) or build the grep pattern accordingly.
