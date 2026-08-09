# split the help table instead of raising the file_lines threshold

## Summary
`src/help.rs` was the largest single number left in `quality-report.json`: 432 counted lines
against a threshold of 300, `file_lines +132`, and **nothing else** — its three functions are all
under every limit. Roughly 315 of those lines are the `const HELP` table of verb text.

The obvious move was to raise the threshold, and it was investigated first. It does not work:
ratchet's `thresholds` are **per category, not per file**, so the threshold has to clear the
largest file. Raising `file_lines` to 433 does not exempt `help.rs` — it zeroes the whole
category:

| threshold | `file_lines` total | still guarded |
|---|---|---|
| 300 (today) | 296 | cli/mod.rs, diff.rs, discovery.rs, help.rs |
| 350 | 126 | cli/mod.rs, diff.rs, help.rs |
| 400 | 32 | help.rs only |
| **433** | **0** | **nothing — any file may reach 433 unchallenged** |

That is the opposite of what the ratchet is for. `exclude: ["src/help.rs"]` is the targeted lever
ratchet does offer, but it drops the file from *every* category — and `for_verb` is cyclomatic 9
against a limit of 10, `wrap` cognitive 8 against 10, so the next edit to either would go
unnoticed.

So: split, and change no threshold.

`src/help/` is now the renderer plus **three tables grouped the way `cli::dispatch` groups the
same verbs** — `edit` (new, mv, start, review, done, set, dep, label), `read` (show, path, which,
list, tree, ready, next, deps, changelog, diff), `maintain` (check, summary, html, repo, init,
version). Mirroring the dispatcher rather than inventing a grouping means adding a verb touches
the matching pair, and it is the difference between the table landing at zero excess and landing
at +20 as one file.

`const HELP` became `GROUPS: &[&[VerbHelp]]` with an `entries()` iterator over it, which is the
only code change — the table text itself moved verbatim.

## Acceptance criteria
- [x] `src/help` gone from every category of `quality-report.json`
- [x] **no threshold moved**, and no other file made worse
- [x] `trck --help` and every `trck <verb> --help` byte-identical
- [x] `cargo fmt`/`clippy`, `cargo test --all`, `conformance/run.py`, `scripts/tests` all pass

## Notes
| category | before | after | delta |
|---|---|---|---|
| `file_lines` | 296 | 164 | −132 |

Every other category is unchanged, no entry anywhere is new or worse, and the `thresholds` block
is untouched — so this needed none of the special handling a threshold edit does (ratchet rejects
one that lands alongside a new violation, which is why it would have been its own PR).

`file_lines` is now `cli/mod.rs +80`, `diff.rs +64`, `discovery.rs +20` — all real code, none of
it data.

**Verified by A/B, not by the test suite.** The two existing tests that would catch a mangled
table (`every_verb_the_binary_offers_has_help` and `documented_options_are_exactly_the_accepted_ones`)
check that the table agrees with the parser — they would not notice a *tagline* or *blurb* being
dropped or reordered, and the split moved 315 lines of exactly that. So the pre-split binary and
the post-split one were both run over `trck --help`, all 24 verbs' `--help`, and an unknown verb:
518 lines, byte for byte identical.

The split itself was done by script rather than by hand, for the same reason — 24 struct literals
retyped is 24 chances to change a word nobody would see fail.
