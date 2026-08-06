# update: the Python engine 404s once ./trck leaves the tree

## Summary
`cmd_update` resolves the newest release tag and then fetches the engine as a **file from the
repository tree**:

```
https://raw.githubusercontent.com/{repo}/{ref}/trck
```

The cutover deletes `./trck`. From that moment, every Python engine in the wild — each one
pointed at the latest release, which will be a Rust one — asks for a path that no longer exists
and reports `update failed (network): HTTP Error 404`. The user is told the network is at fault
for a decision this project made deliberately.

That is the wrong last impression to leave, and it is the one case where the engine being
retired still has a job to do: explaining its own retirement. The Rust engine already answers
`update` with the upgrade path rather than doing anything (`#9898xru`); the final Python engine
should answer the same way once the tag it would fetch is past the end of its line.

Note what this must *not* do: fetch a shim. The download is accepted if it compiles and contains
`__version__`, so a stub that merely explains itself would pass validation and overwrite a
working engine with something that cannot run a tracker.

## Acceptance criteria
- [ ] The final Python release detects that the newest release is past the Python line and
      prints the install path — package manager, or the install script — instead of fetching.
- [ ] It exits non-zero or zero deliberately, not by accident; whichever it is, a script that
      runs `trck update` unattended behaves sanely.
- [ ] It never replaces the running engine with anything that is not a working engine.
- [ ] `--ref` still works, so someone pinning an old Python version can still get it.
