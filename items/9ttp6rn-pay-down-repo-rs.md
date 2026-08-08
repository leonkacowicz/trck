# pay down repo.rs

## Summary
With the gutter cleared (`#nhscdux`), `src/repo.rs` became the worst file left: 164 total excess
across five of the six categories, 465 lines against a threshold of 300, and four functions over
the complexity limits.

It read as one file because `repo` reads as one verb group. It is not one job — it is four
unrelated ones that happen to share a namespace, and the seam was already visible in the function
names: git integration (`setup-git`, `install-hook`), a one-shot layout migration, the merge
drivers, and `normalize`. The file became a directory with one module per verb, plus the two
things more than one of them needs — `git.rs` for talking to git at all, and `attributes.rs` for
the committed half of `setup-git`.

Splitting the file is what paid for splitting the functions inside it, the same lesson every
paydown before this one learned.

Three splits are worth more than the metric they moved:

- **`migrate-layout` now plans, refuses, then acts.** It was one 62-line function that
  interleaved classifying files with moving them, safe only because every refusal happened to
  come before the first `rename`. That ordering was a property of how the code was written, not
  something the structure enforced. A `Plan` is now built without writing anything, `refuse`
  rejects a drifted or blocked tracker, and only then does `apply` touch the disk — so a
  half-migrated tracker is not a state the verb can reach.
- **`setup-git` says out loud that it has two halves.** The doc comment always did; the code
  was one function. `declare` writes the shared `.gitattributes`, `register` writes the
  per-clone `.git/config`, and the verb is the two of them plus the note explaining why the
  distinction exists.
- **`gitattributes_update` lost its two accumulators.** 52 lines carrying `changed`, `missing`,
  `last` and `header_at` at once. `ours_at` answers which existing line we may replace,
  `refresh` overwrites one and reports whether it wrote, `splice_in` places what is missing —
  and the top-level function is now readable as the three-step rule the doc describes.

`install-hook` split along the same lines: `hooks_dir`, `tracker_rel`, `hook_body` and
`staged_guard`, which is where the root-tracker special case now lives with the explanation of
why it exists.

## Acceptance criteria
- [x] every `src/repo.rs` entry gone from all six categories of `quality-report.json`
- [x] no threshold moved, and no other file made worse to pay for it
- [x] `install-hook`, `setup-git` and `migrate-layout` covered by tests **before** the refactor
- [x] `cargo fmt`/`clippy`, `cargo test --all`, `conformance/run.py`, `scripts/tests` all pass

## Notes
Every category that `repo.rs` touched improved, and no threshold moved:

| category | before | after | delta |
|---|---|---|---|
| `file_lines` | 641 | 559 | −82 |
| `function_cyclomatic` | 279 | 246 | −33 |
| `file_functions` | 219 | 201 | −18 |
| `function_cognitive` | 152 | 135 | −17 |
| `function_lines` | 46 | 32 | −14 |

`src/repo/` is `mod.rs` plus `git`, `attributes`, `setup`, `hook`, `migrate` and `drivers` —
seven files against a `module_files` threshold of 20, and `src/` itself drops from 15 files to
14.

**The tests came first, and they had to.** `gitattributes_update` was well covered as a pure
function, but `install-hook`, `setup-git`'s git half and `migrate-layout` had **no test at all** —
and those are the three functions this change rewrites hardest. Refactoring a verb that moves
files on disk, with its error paths unexercised, is not something a golden diff can rescue
afterwards. So:

- **7 conformance fixtures for `migrate-layout`**: the dry run names every move, a dry run moves
  nothing (proven by the tracker still being refused afterwards), the real run moves bodies into
  `items/`, the tracker is usable again after it, both refusals (index/folder drift, occupied
  destination) exit non-zero with their message, and a flat tracker is a no-op.
- **`tests/git_hooks.rs`, 8 tests** against real git repositories, following `git_merge.rs`:
  `setup-git` declares the drivers and registers them with an absolute engine path, is
  idempotent, and adds its rules beside a user's own; the installed hook is executable, stops a
  commit that breaks the tracker, lets an unrelated commit through, and fires on any staged
  change when the tracker is the repo root; both verbs refuse outside a repository.

These cannot be conformance fixtures: the runner's `setup` lines exec only the trck binary, so a
fixture has no way to `git init`.

**This covers two of the three bullets in `#38qfknm`** (install-hook end to end, setup-git's git
half). That issue stays open for the third — `diff` against git revisions — and its body records
what landed here.

One test I did not keep: an assertion that `migrate-layout` leaves a status folder holding an
unrelated file in place. The behaviour is real and `remove_dir` gives it for free by refusing,
but no fixture can observe the surviving file — the conformance goldens cover stdout, the index
and the summary, not an arbitrary path — so asserting it would have meant a test that passes
without checking anything.
