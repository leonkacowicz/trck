## Summary

The vendored-engine feature has been removed, so `trck update` no longer has an engine artifact to update and the verb is now meaningless. Remove it from the command surface.

## Acceptance criteria

- [ ] Remove `update` from CLI parsing, dispatch, and help output.
- [ ] Remove the update implementation and its command-specific tests and fixtures.
- [ ] Remove documentation that advertises `trck update`.
- [ ] Treat `trck update` like any other unknown verb.
- [ ] Keep the remaining test suites and generated quality report current and passing.

## Notes

Supersedes #5jhvpz4, which proposed strengthening the obsolete update path.
