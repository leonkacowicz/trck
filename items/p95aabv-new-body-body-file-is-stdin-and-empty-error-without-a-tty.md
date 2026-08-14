# new: --body, --body-file (- is stdin) and --empty; error without a TTY

## Summary
Follow git's model rather than inventing three flags:

| invocation | behaviour |
|---|---|
| `--body TEXT` | inline, like `git commit -m` |
| `--body-file PATH` | file, like `-F`; **`-` means stdin**, so stdin is not a separate mode |
| `--empty` | deliberate title-only issue |
| no flag, no TTY | **error naming the flags** |

That last row is what makes `trck new` safe for agents and CI: non-interactively it must fail
loudly rather than block on an editor that will never open, or silently file an empty body.

Independent of the branch move — this improves today's tracker and can land first.

## Acceptance criteria
- [ ] `--body`, `--body-file` and `--body-file -` all produce the same issue for the same text.
- [ ] `--empty` files a title-only body from the template's H1 and nothing else.
- [ ] The flags are mutually exclusive, with a clear error when combined.
- [ ] No flag and no TTY errors naming all three flags and exits non-zero, having created nothing.
- [ ] Conformance fixtures cover each form, since all of it is user-visible.

## Notes
Also removes the current two-step dance where `new` prints a path the caller then has to open and fill in.
