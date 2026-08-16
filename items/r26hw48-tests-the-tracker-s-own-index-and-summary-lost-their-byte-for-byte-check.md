# tests: an adversarial index fixture, not the real tracker

## Summary

Two unit tests parsed this repository's live `index.jsonl` and required the bytes back —
`parse_index` → `render_index` for one, `generate_summary` against the committed
`SUMMARY.md` for the other. The flip took that data out of the tree, both tests fell back
to `examples/action-game` in silence, and the coverage went from 289 rows to 35 with
nothing turning red.

The first instinct was to get those 289 rows back. **Measuring them says otherwise.** Across
291 rows of real tracker:

| shape | count |
|---|---|
| title containing `"` | **0** |
| title containing `\` | **0** |
| title containing a newline | **0** |
| astral-plane character | **0** |
| title containing `[` or `]` | 1, accidental |
| title containing `\|` | 1, accidental |
| non-ASCII | 26, all em dashes |

The two characters a JSON serialiser actually breaks on appear **zero** times, and the two
that break a markdown link and a markdown table appear once each by luck. Real data is
*arbitrary*, not *adversarial*: it contains what someone happened to type, which is not the
set that pins a serialiser. What it did cover well is field breadth — all 17 fields appear
somewhere — and that fits in one deliberate row.

It was also a poor test on its own terms: the fixture was live mutable data, and a failure
would have meant "run `trck repo normalize`" rather than "there is a bug".

## What to build

One committed index, hand-built to be hostile, replacing the real-tracker arm of both tests:

- a title containing `"`, `\`, and an escaped newline
- a title containing `[`, `]` and `|` — the markdown link and table breakers, which
  `SUMMARY.md` renders unescaped
- an astral-plane character (beyond the BMP, so a surrogate pair in UTF-16 terms)
- every field populated across the corpus: `parent`, `depends_on`, `labels`, `points`,
  `spec`, `review_url`, `resolution`, `closed`, `started`, `created`, `component`, `kind`
- an empty `labels` list *and* an absent one, which must not render identically in the index
- a custom field with an awkward key
- an epic nested three deep, so the rollup has something to recurse through

Round-trip it, and render its summary against a committed golden.

## Acceptance criteria
- [ ] A committed fixture index holds every shape in the list above.
- [ ] `parse_index` → `render_index` is byte-identical on it.
- [ ] `generate_summary` matches a committed golden for it.
- [ ] Neither test names `issues/` any more, and neither depends on data that changes when someone files an issue.
- [ ] Each new shape is justified where it sits — a fixture nobody can read is a fixture nobody will maintain.

## Notes

The other half of the original issue — asserting that the *committed* tracker is canonical —
is a real check but a data check, not a unit test, and it belongs in the workflow that
already fetches the branch. Split out as its own issue.
