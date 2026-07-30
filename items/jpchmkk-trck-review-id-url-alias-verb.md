# trck review ID [URL] alias verb

## Summary
A third alias verb beside `start`/`done`, driven by the new `"review": "in-review"`
entry in `aliases`:

```
trck review 7                                    # -> in-review
trck review 7 https://github.com/o/r/pull/12     # -> in-review, and links the PR
```

Delegates to `cmd_mv` with `status=<alias target>, pr=<url>` — one move, one `finalize`,
one line of output. The optional URL is the point of the verb: the moment a PR exists is
the moment both facts are known.

## Acceptance criteria
- [ ] `review ID` moves to `in-review`; `review ID URL` also sets `pr`
- [ ] A non-URL positional is rejected before anything is written
- [ ] A config without the `review` alias errors with the `trck mv` hint (as `start` does)
- [ ] Leaves the tracker `check`-clean

## Notes
Aliases stay hardcoded subparsers. Making them data-driven from `trck.json` is a
worthwhile separate change — not this one.
