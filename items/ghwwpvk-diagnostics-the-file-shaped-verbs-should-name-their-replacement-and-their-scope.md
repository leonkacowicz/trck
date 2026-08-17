# diagnostics: the file-shaped verbs should name their replacement and their scope

## Summary

`path`, `which` and `list --paths` refuse against a ref-backed tracker with an accurate
message and a non-zero exit:

```
$ trck path ubvkhds
error: the tracker is git ref 'trck-issues', which has no files on disk
```

That says why it failed and never prints a path that is not there — the trap this wording
already exists to avoid. What it does not say is **what to do instead**. The workflow those
three verbs served, `rg -l 'query' $(trck list --paths) | trck which`, is now
`trck list --contains 'query'` (#ubvkhds), and nothing in the failure points at it. Someone
following an old habit or a stale README reaches a dead end with no forward direction.

The same gap runs the other way: nothing in `--help` says these three are for a tracker that
is a **directory**. They are not deprecated and not going away — #r79v4va was closed
`wontfix` precisely because they still work, and removing a working feature from a supported
storage mode to tidy up this repository would have been a bad trade. But a reader has no way
to learn which mode they apply to except by running one and reading the error.

## What to change

- The refusal names the replacement: `… has no files on disk; use \`list --contains TEXT\` to
  search bodies`. One clause, on the message all three already share.
- `path`, `which` and `list`'s `--paths` say in their help that they need a tracker directory,
  and what to use when there is not one.
- Check the wording is right for the *other* callers of that message. It comes from the
  fallible path accessors in `src/discovery/content.rs`, so anything else reaching them gets
  the new clause too — it should read sensibly there or the clause belongs at the call sites.

## Acceptance criteria
- [ ] The refusal from `path`, `which` and `list --paths` against a ref-backed tracker names `list --contains`.
- [ ] `trck path --help`, `trck which --help` and `list`'s `--paths` line say they require a tracker directory.
- [ ] Every other caller of the same diagnostic was checked, and either reads correctly with the new clause or does not receive it.
- [ ] Conformance covers the new wording, since it is behaviour a user reads.

## Notes

Deliberately **not** part of the storage epic. That epic moved this repository's tracker to a
ref and is finished; this is about how the engine explains itself to everyone else, and it
would be worth doing even if this repository had never migrated.

Third instance of the same class, which is worth noticing: `trck version` printed the tracker
for a directory and **nothing** for a ref, so the one command that could answer "where is my
tracker" was silent for the case that needed it (#8yg822w); `repo setup-git` answered `not a
git repository` when the repository was right there and it was the tracker that had no
directory (#mhc8k3k); and this. Each was fixed or filed on its own. If a fourth turns up, the
pattern — *a verb reports the symptom it can see rather than the cause it cannot* — is worth
one issue of its own rather than a fourth patch.
