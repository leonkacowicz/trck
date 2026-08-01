# completion: __complete callback — parse the command line, walk the argparse tree, emit candidates

## Summary
The one piece of real logic: a hidden verb that takes the whole current command line plus the
index of the word being completed, works out what belongs there, and prints the candidates one
per line (value and optional description). Every shell stub calls this and does nothing else.

Resolution walks the parser from `build_parser()`: pick the subparser named by the first
non-flag word (`repo` nests a second level), then decide between "a flag name" (the word starts
with `-`), "a flag's value" (the previous word is a value-taking option) and "the next
positional". The kind of value comes from [[k44jft7]]; the values themselves from [[c37tmn5]].

## Acceptance criteria
- [ ] Takes the full line, not just the current word, and re-implements no argparse parsing that
      the tree can answer.
- [ ] Handles: verb position, `repo` sub-verbs, long flags, flag values, positionals by index,
      and `--` handling.
- [ ] Reads `--dir` out of the line *before* resolving the tracker, so completion in
      `trck --dir other/issues show <TAB>` offers `other/issues`' ids; precedence
      (`--dir` > `$TRCK_DIR` > walk-up) matches a real invocation by calling the same helper.
- [ ] Prints nothing and exits 0 on *any* failure — no tracker, malformed line, unreadable
      index, unexpected exception. Nothing ever reaches stderr.
- [ ] Never mutates: no `SUMMARY.md` regeneration, no index rewrite, no file created.
- [ ] Tests drive it as (words, cursor index) → expected candidates, covering each branch plus
      the no-tracker and malformed-line cases.

## Notes
- The silent-failure wrapper is not optional politeness: a stray stderr line from a completion
  callback corrupts the bash prompt and is miserable to diagnose. Test it explicitly.
- Hidden from `--help` (`argparse.SUPPRESS`) — it is a protocol, not a user-facing verb.
- Needs [[k44jft7]] and [[c37tmn5]]; blocks [[wct2nav]]. Part of [[9echsrh]].
