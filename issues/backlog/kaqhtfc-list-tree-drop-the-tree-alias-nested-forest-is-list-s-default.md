# list/tree: drop the tree alias (nested forest is list's default)

## Summary
`tree` is a pure argparse alias of `list` (`aliases=["tree"]` on the `list`
subparser) — `cmd_list` never branches on the invoked name. The nested forest is
already `list`'s default render and `--flat` already gives the flat view, so
`tree` adds no behaviour. Drop it so there's one command, which also keeps the
upcoming `--json` story unambiguous (`list --json` = nested, `list --flat --json`
= flat) instead of needing a separate `tree --json` shape.

## Acceptance criteria
- [ ] `aliases=["tree"]` removed from the `list` subparser.
- [ ] `tree` is no longer an accepted subcommand (argparse rejects it).
- [ ] The "`tree` is an alias for this command." line is removed from the `list` description.
- [ ] Any docs/help/README references to `tree` as a command are removed or repointed to `list`.
- [ ] A test asserts `tree` is rejected and `list` still renders the nested forest.

## Notes
Engine: subparser at `trck` ~line 2041; handler `cmd_list` ~line 1455. No
behavioural code change in `cmd_list` — this is parser + help/doc cleanup only.
`issues/CLAUDE.md` lists `trck tree` in the common-verbs table; update it too.
