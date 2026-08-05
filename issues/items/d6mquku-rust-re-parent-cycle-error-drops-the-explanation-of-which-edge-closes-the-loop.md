# rust: re-parent cycle error drops the explanation of which edge closes the loop

## Summary
Both engines refuse a re-parent that would create an effective cycle, and both exit 1. The
messages differ in the part that tells you what to do about it:

```
Python: error: this change would create an effective dependency cycle: #aaaaaaa -> #aaaaaaa;
        authored: #bbbbbbb -> #aaaaaaa; #aaaaaaa inherits #bbbbbbb's deps
Rust:   error: re-parenting #aaaaaaa would create an effective dependency cycle: aaaaaaa
```

Python names the **authored** edge and the inheritance that lifts it, which is what points at the
`dep --remove` that would resolve the conflict. Rust reports only the node, so the user is told
there is a cycle but not which edge makes it one. Rust also prints the id bare where Python
prefixes `#`.

`dep --add` on the same tracker already produces a detailed message in Rust — it is specifically
the re-parent path (`set --parent`) that is terse.

## Acceptance criteria
- [ ] The re-parent refusal names the authored edge and the inheritance path, as `dep --add` does.
- [ ] Ids are `#`-prefixed, consistent with every other diagnostic.
- [ ] `error-reparent-that-creates-an-effective-cycle` is promoted from exit-code-only to
      asserting stderr.

## Notes
Low priority — the guard works and the tracker is protected; only the diagnostic is thinner.
Surfaced by `run.py --compare-bin` while converting the error-path tests (#582stb4). Same family
as [[gw9qnmw]] and [[6vpkjxg]]: diagnostic-quality divergences rather than behavioural ones.
