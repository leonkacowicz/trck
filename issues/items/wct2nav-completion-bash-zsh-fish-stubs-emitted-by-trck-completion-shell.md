# completion: bash/zsh/fish stubs emitted by 'trck completion <shell>'

## Summary
The three thin shells over the callback, plus the verb that prints them. Each stub captures the
current command line, hands it to `trck __complete`, and feeds the result back to its shell's
completion machinery — `complete -F` + `COMPREPLY` for bash, `#compdef` + `_describe` for zsh,
`complete -c trck -f -a '(...)'` for fish. They contain no knowledge of trck's verbs, so they
never need regenerating when the CLI grows.

The scripts live as string constants in `templates.py`, like the other emitted artifacts.

## Acceptance criteria
- [ ] `trck completion bash|zsh|fish` prints the corresponding stub; an unknown shell errors
      with the supported list.
- [ ] zsh and fish show the id titles as descriptions; bash degrades to bare values.
- [ ] Words containing spaces, quotes or `!` complete correctly (title descriptions in
      particular are full of them).
- [ ] Sourcing a stub in a repo with no tracker leaves the prompt usable and silent.
- [ ] Tests assert the emitted text for each shell (golden strings) — real shells are not
      spawned in the suite.

## Notes
- Manual smoke test per shell is still worth doing once before closing; the golden tests only
  prove the text is stable, not that the shell accepts it.
- Needs [[qhf5fa2]]; blocks [[vyg43dm]]. Part of [[9echsrh]].
