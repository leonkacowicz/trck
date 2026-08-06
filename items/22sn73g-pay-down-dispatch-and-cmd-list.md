# pay down dispatch and cmd_list

## Summary
The two worst entries in the quality baseline, and the first test of whether the ratchet
guides toward anything useful. It did — by refusing the obvious fix.

`dispatch` was one `match` over two dozen verbs; `cmd_list` was a ten-condition filter closure
with output-format selection wrapped around it. Splitting each along its natural seams cut
what it was supposed to cut. It also grew `file_lines` and `file_functions`, because more
functions in the same file is what splitting a function means, and the ratchet blocked it:
categories do not offset each other.

The move that satisfies both is the one worth making anyway — put the halves in different
files. `cli.rs` and `query.rs` became module directories, which drops `src/` from 21 files to
19 and takes `module_files` to zero as a side effect.

Every category improved:

| category | before | after |
|---|---|---|
| `file_lines` | 2065 | 1554 |
| `function_cyclomatic` | 467 | 421 |
| `function_lines` | 233 | 184 |
| `file_functions` | 390 | 362 |
| `function_cognitive` | 300 | 294 |
| `module_files` | 1 | 0 |

## Acceptance criteria
- [x] `dispatch`: cyclomatic 43 over → 1, and out of the length and cognitive lists entirely.
- [x] `cmd_list`: cyclomatic 37 over → 8, likewise.
- [x] Behaviour unchanged, and demonstrated rather than asserted — 182 tests and 228
      conformance fixtures pass untouched. No fixture was regenerated, which is the point:
      not one byte of output moved.
- [x] No category regressed, so the ratchet passes rather than needing an exemption.
