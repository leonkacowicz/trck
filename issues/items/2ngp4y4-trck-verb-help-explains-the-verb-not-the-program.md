# trck <verb> --help explains the verb, not the program

## Summary
`trck new --help` printed the same paragraph as `trck --help`: what trck is in general, and
the list of verbs. That is the least useful thing to say to someone who has already chosen a
verb and is asking how to call it.

Every verb now has its own help — usage line, what it does, its arguments and options, and an
example where one earns its place. The text is **inherited rather than invented**: it comes
from the argparse definitions the previous engine carried, where each option already had a
sentence written for it. Reinventing them would have quietly changed what the tool claims to
do, and there was no reason to.

What keeps it true is a test tying the table to the parser in both directions: a verb cannot
document a flag it would refuse, and cannot accept one it leaves unmentioned. That found two
real defects the moment it ran — `deps` inheriting a `--graph` the engine does not accept, and
`diff` accepting `--from` with nothing saying so.

## Acceptance criteria
- [x] `trck <verb> --help` describes that verb; `trck --help` still describes the program.
- [x] An unknown verb falls back to the program's help rather than erroring — someone reaching
      for help is already saying they do not know what to type.
- [x] Help and parser are tested against each other in both directions.
- [x] `repo` lists its subcommands; `tree` points at `list` rather than duplicating fifteen
      filter flags into a second copy that would drift.
- [x] Nothing renders wider than it should, asserted rather than eyeballed.
