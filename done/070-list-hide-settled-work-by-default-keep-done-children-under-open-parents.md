# list: hide settled work by default (keep done children under open parents)

## Summary
`trck list` showed every issue, so long-settled done work drowned out active work.
Filtering `--status '!done'` over-corrected: it dropped the done children of an open
epic, losing the progress context you want when reviewing how far an epic has come.

The default `list`/`tree` now hides **settled** work: a terminal issue is shown only
while it is still open or sits directly under a non-terminal parent. So an open epic
keeps its done children as context, while a fully-done subtree and standalone done
tasks drop off. The forest's `match_closure` still pulls open ancestors back as dimmed
context, so open work under a done parent is never orphaned.

## Acceptance criteria
- [x] Default view drops terminal issues whose parent is also terminal (or absent).
- [x] An open (non-terminal) parent keeps its terminal children visible.
- [x] `--all` shows everything, including settled subtrees.
- [x] An explicit `--status` (e.g. `--status done`) bypasses the default prune entirely.
- [x] The prune applies in both the nested forest and `--flat` views.
- [x] Behaviour is data-driven off the `terminal` status role, not the literal `done` name.

## Notes
- Engine: `cmd_list` gains a `prune_settled` guard + `settled(r)` helper; `--all` flag added.
- Tests: `tests/test_list_default_filter.py`.
- Docs: README `list` section + CLI examples epilogue updated.
