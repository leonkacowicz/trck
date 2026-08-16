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
- [ ] Case-insensitive substring by default; `--regex` opts into regex.
- [ ] Honors metadata filters (at least `--status`) to narrow the search set.
- [ ] Prints in the same one-line-per-issue format as `list`; empty result prints nothing.
- [ ] Tests cover: body hit, title hit, no hit, regex match, filter intersection.

## Notes
Read body text from the issue markdown files. No external grep binary at run time — the
matching happens in-process.

## Resolution
Resolved by **composition**, not a built-in `search`/`grep` verb. The matching
primitive (substring/regex) is commodity; the value `trck` uniquely adds is
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
  capable than the proposed built-in matcher); the engine stays free of a search verb.
- honors metadata filters → `list --paths` inherits every `cmd_list` filter.
- same one-line-per-issue format / empty prints nothing → `trck which` uses `print_rows`.
- the engine gains no runtime dependency on an external tool — the user brings their
  own `rg`/`fzf`; nothing is shelled out internally.

## Addendum — 2026-08-16: the premise expired
The resolution above rests on one fact: *issue bodies are plain files the search tool
already on the machine can read.* The move to the `trck-issues` ref (#sqzr7nk) made that
false. There are no body files in the working tree, and all three primitives this issue
added resolve through `Ctx::dir`, which answers a ref-backed tracker with "the tracker is
git ref '…', which has no files on disk" — so `list --paths`, `path` and the `rg … | trck
which` pipeline refuse rather than search.

What replaces it is **not** the `trck search` verb this issue rejected. It is a filter on
`list` — `--contains PATTERN`, tracked as #ubvkhds. That keeps the part of the reasoning
that still holds: the matching primitive is commodity, and on a ref it is `git grep -l -F
-i PAT <rev> -- items/`, one process reading blobs with no checkout. git grep does not
disappear from the design; it moves inside the engine and stops being something the
operator types. What trck still uniquely adds is the same thing it added here — mapping a
hit back to an issue record — except that as a filter it also inherits every metadata
filter, the nested forest, the ordering and `--json`, none of which the pipeline could
compose with.

So: a premise change, not a change of mind. This issue stays **done** — the composition it
built was right for a tracker made of files, and it is `#ubvkhds` that owns the ref-backed
answer and the retirement of `path`/`which`/`--paths` that follows it.
