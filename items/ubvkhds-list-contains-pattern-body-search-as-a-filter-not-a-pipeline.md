# list --contains PATTERN: body search as a filter, not a pipeline

## Summary
The flip took full-text body search with it. `path`, `which` and `list --paths` all
resolve through `Ctx::dir`, which answers a ref-backed tracker with *"the tracker is git
ref '…', which has no files on disk"* — so the documented one-liner,
`rg -l 'query' $(trck list --paths) | trck which`, has no files to hand `rg` and no way
back. README's **Full-text body search** section (README.md:442) still prescribes it.

Put the capability back as a **filter on `list`** rather than a pipeline between three
verbs: `trck list --contains TEXT`, a case-insensitive literal substring over the issue's
body file. That is strictly better than what it replaces — every existing filter composes
natively (`--status`, `--label`, `--parent`, `--field`, `--sort`), and the nested forest,
the ordering, `--json` and `--show-field` all come free, none of which the pipeline could
do without pathspec gymnastics.

The body file opens with its `# Title` heading, so `--contains` subsumes `--match` for
free. Keep the pair distinct anyway: `--match` is title-only, `--contains` is anywhere in
the body.

## Acceptance criteria
- [ ] `trck list --contains TEXT` keeps only issues whose body contains TEXT, matched
      case-insensitively as a literal substring — the same semantics `--match` has on the
      title, so there is no regex surface to own.
- [ ] Composes with every existing filter and output mode: `--status`, `--label`,
      `--parent`, `--field`, `--flat`, `--json`, `--show-field`, `--sort`.
- [ ] Answers identically against a ref-backed and a directory-backed tracker — or the
      difference is deliberate, documented and covered by a test.
- [ ] Costs **one** git invocation per `list`, not one per row (see Notes).
- [ ] A pattern that matches nothing prints nothing and exits 0; it is not an error.
- [ ] Conformance: body hit, title-heading hit, no hit, intersection with `--status`, and
      `--contains` against a ref-backed tracker.
- [ ] README's **Full-text body search** section rewritten around the flag.

## Notes
**It has to be a pre-pass, not a predicate.** `RowFilter::keeps` (src/query/list.rs:124) is
pure metadata today — no `ctx`, no I/O — and it should stay that way. A per-row body read
against a ref is one `git show` spawn per row (src/discovery/content.rs:96); this tracker
has ~290 issues. So compute the matching id set once, up front, and have `keeps` test
membership.

**On a ref, the natural way to compute that set is `git grep`:**
`git grep -l -F -i PAT <rev> -- items/` reads the blobs with no checkout, one process, and
the ids come straight off the returned filenames. git grep does not disappear from the
design — it moves inside the engine and stops being something the operator types.

**Two backends, one flag.** Directory-backed trackers still exist for other repos. Either
match in-process there — Rust's `to_lowercase` is Unicode-aware and git's case folding is
not, so the two diverge on non-ASCII — or reach for `git grep --no-index` on that side too
and have one matcher everywhere. Prefer the second: git is already a definitional runtime
dependency, and one matcher cannot drift from itself.

**Deliberately out of scope:** retiring `path`, `which`, `list --paths` and
`src/query/paths.rs`. That follows once this lands and is its own change; do not delete the
escape route before the replacement exists.

This reverses the resolution recorded on #5wbwpjv, which closed body search as
*composition, not a search verb*. That decision rested on "issue bodies are plain files the
search tool already on the machine can read" — the ref falsifies the premise, so this is a
premise change rather than a change of mind. An addendum on #5wbwpjv says so.
