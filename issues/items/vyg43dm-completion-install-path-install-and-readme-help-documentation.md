# completion: install path (--install) and README/help documentation

## Summary
Getting the stub from [[wct2nav]] into the user's shell. At minimum, documented redirects
(`trck completion bash > ~/.local/share/bash-completion/completions/trck` and the zsh/fish
equivalents); optionally a `--install` that picks the conventional per-shell directory and
writes the file itself.

Also the caveat that will otherwise generate confused reports: completion binds to the command
*name*, so it fires for a `trck` on `PATH` and not for `./trck` or a vendored `issues/trck`.

## Acceptance criteria
- [ ] README documents the per-shell install line and where each shell looks.
- [ ] The `completion` verb's `--help`/epilog carries the same one-liner, so it is discoverable
      without the README.
- [ ] The name-binding caveat is stated, with the alias workaround for vendored engines.
- [ ] If `--install` ships: it prints the path it wrote, refuses to clobber without `--force`,
      and is covered by a test against a temp HOME.

## Notes
- `--install` is optional scope — decide when the stubs land whether it earns its complexity or
  a documented redirect is enough.
- Needs [[wct2nav]]. Part of [[9echsrh]].
