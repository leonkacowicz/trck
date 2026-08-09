# pay down the per-row checks

## Summary
`src/validate/mod.rs` carried 55 excess with **no file-level violation at all** — 262 lines and
19 function spaces, both under threshold. All of it was three functions, and `check_row` was the
worst function left in the tree: 64 lines, cognitive 25, cyclomatic 20.

That "under threshold" is the interesting constraint. `file_functions` was at 19 of 20, so
decomposing anything would have blown it — the split was forced by the decomposition rather than
by the file's size. Three modules now:

- `row.rs` — the per-row checks.
- `cycle.rs` — explaining an effective cycle.
- `mod.rs` — finding the files to check against, and the list of passes.

**`check_row` got the treatment `validate` itself got in `#y5a9jwj`**: it was a run of
independent checks, so it is now the list of them, each named for what it checks — `check_naming`,
`check_vocabulary`, `check_points`, `check_closure`, `check_review_url`, `check_custom_fields`.

**The order of that list is the contract.** `check` prints errors as they are pushed, and the
conformance suite compares stderr literally, so grouping the checks by *topic* rather than by
*position* would have silently reordered the output of any row with more than one problem. The
grouping chosen is the one that is both coherent and order-preserving: `check_closure` covers the
resolution word and then the `(status, closed, resolution)` tuple, which is the same story anyway.

`describe_cycle` split along its own explanation: `closed_loop` (the loop with its first node
repeated so the closing edge is explained too), `witness` (the authored edge that makes one hop
hold), `explain_edge`, and `assemble`. `scan_files` gave up its filename parsing to
`issue_filename`, which is the gate that keeps a stray `README.md` in `items/` from being read as
an issue and then reported as one missing its index row.

## Acceptance criteria
- [x] every `src/validate/mod.rs` entry gone from all six categories of `quality-report.json`
- [x] no threshold moved, and no other file made worse to pay for it
- [x] every diagnostic `check` can emit captured before the change and unchanged after,
      **including the order** a multi-problem row reports them in
- [x] `cargo fmt`/`clippy`, `cargo test --all`, `conformance/run.py`, `scripts/tests` all pass

## Notes
The three categories the file touched improved, and the deltas sum to exactly its former 55:

| category | before | after | delta |
|---|---|---|---|
| `function_cognitive` | 86 | 58 | −28 |
| `function_lines` | 15 | 1 | −14 |
| `function_cyclomatic` | 168 | 155 | −13 |

No threshold moved, and no entry anywhere in the report is new or worse. **`function_lines` is now
a single entry across the whole codebase.** What remains under `src/validate/` is
`checks.rs::check_references` at cognitive 11 (+1) — untouched, pre-existing, in a different file.

**Goldening the diagnostics found two holes in my own harness first.** 30 malformed trackers, one
per case: every per-row error, both dangling-reference errors, three cycle shapes, the rollup
mismatch, the unfinished-dependency warning, the vestigial-config warnings, the fatal
duplicate-id-on-disk path, and positive controls for a terminal row that legitimately carries
`closed` and `resolution` and for a parent pinned with `manual_status`.

Two of the first drafts were not testing what they claimed:

- The `bad slug` case used the file `aaaaaaa-Bad Slug.md`, which `issue_filename` correctly refuses
  — so `check_row` reported "no markdown file" and returned, and the slug check never ran.
- The **multi-error ordering case** had the same flaw, which is much worse: the one fixture whose
  entire purpose was to pin the output order was short-circuiting on the first check.

Both now use a valid filename whose slug merely disagrees with the index, and the ordering fixture
pins all twelve per-row errors in sequence. That is the assertion this refactor actually needed.

**Coverage.** 3 unit tests became 22. `mod.rs` had only ever tested `is_slug`, `is_field_key` and
`repr` — the three smallest things in it — while `check_row` and `describe_cycle`, the two that
carried the complexity, had none. The additions cover each per-row check in isolation, that a
missing file stops the rest, that the closure tuple reports its two halves separately while a
terminal row carrying both stays silent, that a bad custom-field key is reported *instead of* the
type complaint rather than as well as, every shape `issue_filename` must refuse, and four cycle
shapes including a one-node loop nobody typed and an unexplainable edge still yielding its chain.

Filed `#mwdg3pv` for something spotted on the way: `validate` and `issue` each carry their own
Python `repr`, and they disagree on escaping quotes in strings. Not fixed here — unifying them
changes `check`'s wording, which belongs in a change whose goldens are about the wording.
