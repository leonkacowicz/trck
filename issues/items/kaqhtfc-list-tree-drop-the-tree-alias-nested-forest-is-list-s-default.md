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
Engine: `list` subparser (with `aliases=["tree"]`) at `src/trck/cli.py:209`;
handler `cmd_list` at `src/trck/cmd_query.py:68`. No behavioural code change in
`cmd_list` — this is parser + help/doc cleanup only.

Doc/help sites to repoint: the "`tree` is an alias for this command." line in the
`list` description (`src/trck/cli.py:226`); `trck tree 4` in the top-level TYPICAL
FLOW epilog (`src/trck/cli.py:73`); `README.md:39`, `README.md:164`, and the
screenshot caption at `README.md:167-168` (the asset is `docs/img/tree.svg`, whose
prompt line reads `$ trck tree` — regenerate or recaption); and the common-verbs
list in `issues/CLAUDE.md`.
