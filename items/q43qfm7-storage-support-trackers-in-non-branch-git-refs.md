## Summary

Allow a ref-backed tracker to live in an arbitrary Git ref namespace such as `refs/trck/issues`, rather than coercing every writable tracker into `refs/heads/*`. This extends the storage work from #sqzr7nk without changing the conventional `trck-issues` branch by default.

## Acceptance criteria

- [ ] Represent the read revision, local writable ref, remote destination ref, and local tracking cache explicitly instead of deriving them from a branch name.
- [ ] Read, write, push, replay, and sync a tracker stored outside `refs/heads/*`.
- [ ] Reject writes through immutable revisions such as commit SHAs and tags with a clear diagnostic.
- [ ] Provide a setup path for adding and fetching the custom refspec in a fresh clone.
- [ ] Detect legacy-branch/custom-ref ambiguity and refuse divergent storage rather than choosing silently.
- [ ] Cover custom-ref discovery, offline writes, contention replay, sync, and fresh-clone behavior in integration tests.
- [ ] Document that ordinary clones do not fetch custom refs and GitHub Actions cannot trigger branch-push workflows from them.
- [ ] Keep `trck-issues` branch behavior compatible unless a separate migration explicitly changes the default.

## Notes

This does not by itself migrate existing trackers or solve repository-level distribution of custom fetch refspecs.
