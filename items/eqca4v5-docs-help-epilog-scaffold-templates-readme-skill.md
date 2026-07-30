# docs: help epilog, scaffold templates, README, skill

## Summary
Teach every surface that documents the vocabulary about the new status, field, and verb:
- `cli.py::TOP_EPILOG` — the `config` line's stated defaults, and a TYPICAL FLOW entry
  for `trck review`.
- `templates.py` — `CLAUDE_MD_TEMPLATE` / `README_TEMPLATE`, which `init` scaffolds into
  consumer repos.
- The repo `README.md`.
- `skills/trck` — the agent-facing skill's command reference.

## Acceptance criteria
- [ ] `--help` and the epilog describe `in-review`, `--pr`, and `review`
- [ ] Scaffold templates mention them; `test_help` / `test_init` expectations updated
- [ ] README and skill reference the new verb

## Notes
`_refresh_managed_docs` only rewrites a scaffolded `CLAUDE.md` the user never edited, so
changing the template is safe for existing consumers.
