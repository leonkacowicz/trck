# pay down the issue record

## Summary
`src/issue.rs` was the worst real target left: 125 total excess across four categories, 530
lines, and it held `from_json` — 67 lines at cyclomatic 28, the single worst function in the
tree.

Reading a row is four steps, and the file had them interleaved. They are now a module each,
which is also the order they run in:

- `row.rs` — the pairs, with duplicates collapsed the way Python collapses them.
- `read.rs` — what each field is, and the migrations an older engine's row needs.
- `coerce.rs` — turning a value into the field's type, or refusing.
- `diagnostic.rs` — the wording of the refusal, which is itself contract.
- `write.rs` — the other direction, and the two forms that are deliberately not alike.
- `mod.rs` — the record, the canonical key order, and the custom-field key rule.

**`from_json` came apart along its own defaults.** The version that got the cyclomatic count
under the limit is also the one that says something the module doc had only asserted: a row that
mentions nothing but the five required fields *is* an issue with every other field at its
default. So `read_required` builds exactly that — defaults spelled out once, in one place —
and `read_defaulted` and `read_optional_strings` then fill in what the row actually said. The
first attempt threaded two intermediate structs and 30 lines of field copying through instead;
it hit the number and was worse code, so it went.

`to_canonical` split the same way, into the five fields that are always written and the rest
that are written only where they differ from a default.

## Acceptance criteria
- [x] every `src/issue.rs` entry gone from all six categories of `quality-report.json`
- [x] no threshold moved, and no other file made worse to pay for it
- [x] every diagnostic `from_json` can emit captured before the change and unchanged after
- [x] `cargo fmt`/`clippy`, `cargo test --all`, `conformance/run.py`, `scripts/tests` all pass

## Notes
Every category the file touched improved:

| category | before | after | delta |
|---|---|---|---|
| `file_lines` | 364 | 296 | −68 |
| `file_functions` | 168 | 147 | −21 |
| `function_cyclomatic` | 217 | 198 | −19 |
| `function_lines` | 32 | 15 | −17 |

No threshold moved. `src/` drops from 13 files to 12, and `module_files` stays at zero.
`function_lines` is now down to a single entry across the whole tree.

**The read order is the diagnostic, and that is what needed protecting.** A row can be wrong in
several places at once and only the first complaint is reported — so reordering the reads is a
silent, user-visible behaviour change that no type checker would catch. Before touching
anything, all **38 diagnostics** `from_json` can emit were captured by feeding malformed rows
through the built binary: every missing field, every wrongly typed one, both list mistakes, the
five non-object shapes, and a deliberately doubly-broken row to pin down *which* field wins.
Plus 11 migration round-trips (`milestone`, `pr`, the legacy status, unknown keys, duplicate
keys, defaults stripped) and every `--json` path over the real tracker. 13,997 lines, byte for
byte identical afterwards.

That capture earned its keep immediately: it showed that a row with both a bad `spec` and a bad
`manual_status` blames `manual_status`, because the original computed `manual_status` before the
struct literal that held `spec`. A natural-looking tidy-up — reading the fields in declaration
order — would have flipped it. The order is now asserted by a test
(`the_first_complaint_follows_the_read_order`) rather than left to survive on luck.

**Coverage.** 13 unit tests became 38, and the additions are mostly cases the split made
nameable: `py_repr` for every JSON shape with Python's spelling (`None`/`True`, single-quoted,
escaped quotes and backslashes), `Row`'s duplicate collapsing and null-is-absent rule, the two
distinct list mistakes, an empty id versus a wrongly typed one, the whole custom-field key
grammar at its edges, every `CANON_KEYS` entry being refused as a custom field, a `null` `pr`
migrating to nothing, and both serialised forms reading back to the same `Issue`.
