# shell completion for bash, zsh and fish

## Summary
`trck` has a wide verb surface and ids that are random 7-char strings — the two things Tab
completion helps most with. Today there is none: every id has to be copied from a `list` run by
hand, and the verb/flag tree is only discoverable through `--help`.

The engine is stdlib-only, so `argcomplete` and friends are out; and the vocabulary is
per-tracker (statuses, priorities, kinds, resolutions all come from `trck.json`), so a static
script with baked-in status names would be wrong for any repo that reconfigures them. Both
constraints point at the same shape: **a hidden callback in the engine does all the thinking,
and each shell gets a thin generic stub that forwards the current command line to it.** One
implementation covers three shells and can never drift from `build_parser()`, which already
holds the verbs, flags and help strings.

## Acceptance criteria
- [ ] `trck completion bash|zsh|fish` prints a stub the user can source or install.
- [ ] Completions cover verbs, flags, and the dynamic values: ids (with titles as descriptions
      where the shell supports them), statuses, priorities, kinds, resolutions, labels.
- [ ] Every candidate list is derived from the parser or from the tracker's own config — no
      hard-coded status/priority names anywhere in the completion code.
- [ ] The callback honours `--dir`/`$TRCK_DIR`/walk-up discovery exactly as a real invocation
      would, and stays silent (empty output, exit 0) when there is no tracker or the line is
      unparseable.
- [ ] Children done.

## Notes
- Children, in dependency order: [[k44jft7]] (annotate the arguments) and [[c37tmn5]] (value
  providers) feed [[qhf5fa2]] (the callback), which [[wct2nav]] (the shell stubs) drives, and
  [[vyg43dm]] documents/installs.
- Rejected alternative: emitting three fully-static scripts with the verb/flag tree baked in.
  Faster per Tab (no interpreter start) but three shell dialects to maintain and three chances
  to drift from `cli.py`. The callback approach pays ~40ms of Python startup per Tab, which is
  not noticeable at the prompt.
- Completion binds to the *command name*, so it only fires for a `trck` on `PATH` — not for
  `./trck` or a vendored `issues/trck`. Worth saying out loud in the docs.
