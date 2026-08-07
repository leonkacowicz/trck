# vocabulary: drop the 'ongoing' status alias

## Summary
`ongoing` was renamed to `in-progress` because it said the wrong thing: it reads as work of
indeterminate duration, something done routinely, when what the status means is that someone
happens to be on it right now and will finish. The rename shipped with a read-side alias so no
tracker broke and no muscle memory had to be retrained — `config::canonical_status` resolves the
old name at every boundary a status arrives through, and nothing ever writes it back. This issue
is the other half: eventually take the alias out.

The alias is the whole compatibility story, so removing it is exactly one deletion plus its call
sites. What decides the timing is how many trackers in the wild still carry `"status": "ongoing"`
in an unconverted `index.jsonl` — a tracker converts itself the first time any verb rewrites its
index, so the population shrinks on its own.

## Acceptance criteria
- [ ] `config::canonical_status` and `LEGACY_IN_PROGRESS` are gone, along with their three call
      sites (`Issue::from_json`, `cli::opts::mv_opts`, `query::list::parse_status_filter`)
- [ ] A stored `"status": "ongoing"` is refused by `check` with the ordinary unknown-status
      diagnostic naming the four current statuses, rather than being silently converted
- [ ] `trck mv ID ongoing` and `trck list --status ongoing` fail the same way
- [ ] There is a documented conversion path for a tracker that still carries the old name —
      `repo normalize` run by the last engine that understood it, or a note in the release
- [ ] The four `*ongoing*` conformance fixtures are replaced by ones asserting the refusal
- [ ] Every doc that still mentions the alias as accepted input is updated

## Notes
Deliberately **not** blocked on anything and deliberately low priority: the alias costs one
function and one branch, so there is no pressure to remove it, and removing it too early is the
only way this change can hurt anyone. Revisit when a release goes out that is far enough past the
rename that an unconverted tracker is implausible.

Where the alias lives today:

    src/config.rs          LEGACY_IN_PROGRESS + canonical_status — the only place the old name is understood
    src/issue.rs           from_json: a stored row reads under the current name
    src/cli/opts.rs        mv_opts: `mv ID ongoing` still moves the issue
    src/query/list.rs      parse_status_filter: `--status ongoing` selects what it became

The one asymmetry worth keeping in mind: the alias is read-only by construction, so a tracker
touched by any mutating verb has already converted. That is what makes this deletion cheap
later — the data problem solves itself, and only the code has to be cleaned up.
