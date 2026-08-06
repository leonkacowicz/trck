# a legacy integer-id tracker gets four wrong errors instead of the way out

## Summary
Point the binary at a tracker from trck's first iteration — numeric ids, issue files named
`001-epic.md` — and it reports this:

```
error: #1 in index but no markdown file on disk
error: #2 in index but no markdown file on disk
error: #001 markdown file on disk but no index row
error: #002 markdown file on disk but no index row
```

Four errors, none of them true. The files are on disk and the rows are in the index; what is
actually wrong is that the naming convention changed, which none of those sentences says. The
engine it replaces recognises the shape and answers in one line:

```
error: legacy integer ids: 2 issue file(s) still named by number (e.g. 001-epic.md) —
integer ids are no longer supported. Convert with scripts/renumber.py (it writes an
old->new map), or stay on trck 0.25.
```

That message is the only pointer to the conversion path, and the README still documents
`scripts/renumber.py` as the way through. So this is not a cosmetic difference in wording: it
is the difference between a user who converts and a user who concludes the tracker is corrupt.

No conformance fixture covers a legacy tracker, which is why the port lost this without
anything failing. That gap is the more useful half of the finding — the diagnostic could be
restored and silently rot again tomorrow.

Found while repointing `scripts/tests/test_renumber.py` at the binary during the cutover
(`#djx63gk`). Its assertion on that message is what failed.

## Resolution — wontfix

Integer-id trackers are no longer supported at all, rather than supported with a diagnostic.
They were trck's first iteration, replaced because two branches running `new` minted the same
number; anything still carrying them predates several format changes, and the engine that
could explain the conversion is being retired anyway.

So the promise goes with the capability: the README's conversion section is removed, and
nothing points at a path the binary does not implement. `scripts/renumber.py` stays in the
repository as a one-shot for anyone who goes looking, undocumented and unsupported.

What a legacy tracker gets now is the generic consistency errors — imprecise, but honest
about the tracker being unreadable by this engine. Anyone who needs the old behaviour can
install v0.25.1, which is still published.
