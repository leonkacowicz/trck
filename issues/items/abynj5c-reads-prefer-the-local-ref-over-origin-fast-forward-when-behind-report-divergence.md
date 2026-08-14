# reads: prefer the local ref over origin, fast-forward when behind, report divergence

## Summary
Reading `origin/trck-issues` unconditionally would hide a write that could not be pushed:
file an issue offline, and `trck list` does not show it. The rule, from the epic's table:

| local vs `origin/trck-issues` | read |
|---|---|
| ahead, or equal | local |
| behind | fast-forward local, read local |
| diverged | local, **and say so** |
| absent | `origin/trck-issues` |

## Acceptance criteria
- [ ] Each of the four cases resolves to the documented ref, under the #A4 harness.
- [ ] The behind case fast-forwards `refs/heads/trck-issues` and does not fetch to discover it.
- [ ] The diverged case reads local and prints a diagnostic naming `trck sync` as the remedy, on stderr, so piped output stays parseable.
- [ ] A read never moves the local ref except by fast-forward.

## Notes
Divergence needs unpushed local work *and* a remote that moved — rare, but it is exactly the state a failed push plus someone else's write leaves behind.
