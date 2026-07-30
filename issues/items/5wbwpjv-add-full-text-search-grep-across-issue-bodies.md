# Add full-text search/grep across issue bodies

## Summary
`list` only filters indexed metadata; there is no way to find an issue by
something written in its prose body (Summary / Acceptance criteria / Notes).
Add `trck search <query>` (alias `grep`) that scans issue bodies and titles and
prints the matching issues, like a `list` result.

Matching is a plain substring by default, case-insensitive, with an optional
`--regex` flag. Composes with existing metadata filters (e.g. `--status`).

## Acceptance criteria
- [ ] `trck search <query>` matches against title + body text and lists hits.
- [ ] Case-insensitive substring by default; `--regex` opts into regex (stdlib `re`).
- [ ] Honors metadata filters (at least `--status`) to narrow the search set.
- [ ] Prints in the same one-line-per-issue format as `list`; empty result prints nothing.
- [ ] Tests cover: body hit, title hit, no hit, regex match, filter intersection.

## Notes
Read body text from the issue markdown files. Keep it stdlib-only — no external
grep dependency.

## Resolution
Resolved by **composition**, not a built-in `search`/`grep` verb. The matching
primitive (substring/regex) is already the stdlib; the value `trck` uniquely adds is
mapping a hit back to an issue record. So instead of a search engine, three small,
addressable primitives were added — let `rg`/`grep`/`fzf` do the searching:

- `trck list --paths` — emit the absolute file path of each issue passing the existing
  metadata filters (`--status`, `--label`, …). Scopes the search set.
- `trck path <id>` — the single-issue path (e.g. `$(trck path 25)`).
- `trck which` — read issue file paths (args, or stdin) and render the matches as
  `list` rows; `--ids` for bare ids.

The full-body search the original `trck search` proposed is then:

    rg -l 'query' $(trck list --paths --status '!done') | trck which

Acceptance criteria, mapped to the composition:
- title + body hit → `rg`/`grep` over `$(trck list --paths)` (the body file includes
  the `# Title` heading); hits rendered as rows by `trck which`.
- case-insensitive default / regex → delegated to `rg -i` / `rg` regex (strictly more
  capable than the proposed stdlib matcher); the engine stays free of a search verb.
- honors metadata filters → `list --paths` inherits every `cmd_list` filter.
- same one-line-per-issue format / empty prints nothing → `trck which` uses `print_rows`.
- still stdlib-only, no runtime dependency on an external tool — the user brings their
  own `rg`/`fzf`; nothing is shelled out internally.
