# pay down finalize, the single write path

## Summary
`finalize` was the worst function left in the tree: 50 lines, cognitive 22, cyclomatic 21. It is
the single write path every mutating verb ends in, so it had accumulated four unrelated jobs in
one body — reset a parent's points, derive a parent's status bottom-up, write the index and
summary, then validate what was written and complain.

Each is now a function named after its job, and `finalize` is the four calls:

```rust
let mut g = Graph::new(rows);
reset_parent_points(&mut g);
derive_parent_statuses(&mut g)?;
write_atomic(&ctx.index_path(), &render_index(&g.rows))?;
write_atomic(&ctx.summary_path(), &generate_summary(&g))?;
report_inconsistencies(ctx, &g.rows);
```

The derivation gained the most from being named. `derived_status` answers *what a parent's status
should be* — or `None` for a leaf, a pinned parent, or one that already agrees — and `set_status`
is the write plus the graph rebuild, with the comment explaining why the rebuild is not optional:
`postorder` walks a snapshot, so a grandparent derived later has to see the status the pass just
wrote. That was the least obvious line in the original and it now has somewhere to live.

`verbs/mod.rs` was at 24 function spaces against a limit of 20, so decomposing anything would
have blown `file_functions` — the split was forced by the decomposition, as with `validate`.
Four new modules, each one thing: `slug.rs`, `write.rs`, `status.rs`, `finalize.rs`. `mod.rs`
keeps the template, `issue_path`, `load_rows`, `resolve_ref` and the re-exports.

## Acceptance criteria
- [x] every `src/verbs/mod.rs` entry gone from all six categories of `quality-report.json`
- [x] no threshold moved, and no other file made worse
- [x] the write path's output — index, summary and the inconsistency report — unchanged
- [x] `cargo fmt`/`clippy`, `cargo test --all`, `conformance/run.py`, `scripts/tests` all pass

## Notes
The three categories the file touched improved, and the deltas sum to exactly its former 29:

| category | before | after | delta |
|---|---|---|---|
| `function_cognitive` | 58 | 44 | −14 |
| `function_cyclomatic` | 155 | 144 | −11 |
| `file_functions` | 136 | 132 | −4 |

No threshold moved, and no entry anywhere in the report is new or worse. What remains under
`src/verbs/` is `edit/set.rs` (+7 `file_functions`, plus three of its functions on cyclomatic) and
`clock.rs::parse_instant` (+8 cyclomatic) — untouched, pre-existing, other files.

**One dead branch went with it.** `slugify` had an `else if c.is_ascii() || !c.is_alphanumeric()`
arm and an `else` arm whose bodies were identical (`pending_dash = true`), so the condition never
decided anything. Collapsed to one `else`, with the reason non-ASCII alphanumerics are dropped kept
as the comment.

`reset_parent_points` also swapped a `Vec::contains` for a `BTreeSet` — it was a linear scan per
row over the parent list, which is quadratic on the whole index and does the same thing.

**Verified by A/B on the write path, since that is what `finalize` is.** A scratch tracker driven
through nine scenarios: every mutating verb in sequence (`new`, `start`, `review`, `done`, `set`,
`dep`, `label`, `mv`, `summary`), a parent's points reset on write, re-parenting resetting the *new*
parent's points, grandparent derivation in one pass, mixed children making an active parent and
all-done a done one, reopening a child reopening the chain, a `manual_status` pin being left alone
and then released with `--auto`, `finalize` reporting an inconsistency it did not cause, and an
orphan body file being warned about rather than fixed. The index and `SUMMARY.md` are dumped in
full after each. 478 lines, byte for byte identical between the pre-split and post-split binaries.

The harness needed one fix before it was worth anything: it printed the scratch tracker's temporary
path, which differs every run, so two captures of the *same* binary did not match. Normalised to
`<TRACKER>` and confirmed stable across runs before being used as a baseline — an unstable golden
is worse than none, because the first diff teaches you to ignore it.

**Coverage.** 3 unit tests became 22. `verbs/mod.rs` tested `apply_status`'s reopening rule,
`slugify`, and `resolve_ref`; `finalize`, `postorder`, the points reset and the status derivation —
the four things that carried the complexity — had none. The additions cover postorder ordering and
its cycle guard, the points reset on both a parent and a leaf, all three derivation outcomes, a
pinned parent, grandparent derivation in one pass, `started`/`closed` each being stamped once and
not restamped, a refused transition leaving the row untouched, `write_atomic` creating a missing
parent directory and leaving no temporary behind, and that `slugify`'s output always satisfies
`check_slug` — which nothing had asserted, though `new` depends on it.
