# restore the coverage the deleted suite carried: hooks, setup-git, git-revision diff

## Summary
The cutover (`#djx63gk`) deleted 706 tests along with the engine they tested. Most of them
went with something that no longer exists — an amalgamation build, a self-updater, a second
implementation — and are no loss. Three areas are a real loss, and were checked one by one
rather than assumed:

- **`repo install-hook`** has no test at all now. Nothing asserts that the hook is written to
  the right path, that it fires only when the tracker is among the staged files, that it finds
  an engine, or that it aborts a commit when `check` fails. It was verified by hand during the
  cutover and works; that is not the same as a test.
- **`repo setup-git`'s git half.** `repo.rs` covers `gitattributes_update` thoroughly — it is a
  pure function — but nothing covers registering the merge drivers in `.git/config`: that the
  driver command is written, that it names `%O %A %B`, that re-running is idempotent, that the
  command points at an absolute engine path rather than leaning on `PATH`.
- **`diff` against git revisions.** `diff.rs` tests the comparison itself, and there is no
  conformance fixture for `diff` at all, so nothing exercises reading an `index.jsonl` out of a
  revision — the part that touches git rather than the part that compares two snapshots.

The pattern is the same in all three: what is missing is exactly what needs a **real git
repository** to test, which is what made those tests awkward to express as conformance fixtures
in the first place. `tests/git_merge.rs` already shows the way — it builds a
repository, branches it, and merges — so this is a matter of extending an established pattern
rather than inventing one.

## Acceptance criteria
- [ ] `install-hook` covered end to end: installed, fires on a staged tracker change, ignores
      unrelated commits, aborts on an inconsistent tracker, and does nothing findable-engine-free
      rather than failing.
- [ ] `setup-git` covered on the `.git/config` side, including idempotence and the absolute
      engine path.
- [ ] `diff` covered across real revisions, not just across two in-memory snapshots.
- [ ] Where the behaviour is user-visible, prefer a conformance fixture; reach for an
      integration test only where a fixture genuinely cannot express a git repository.
