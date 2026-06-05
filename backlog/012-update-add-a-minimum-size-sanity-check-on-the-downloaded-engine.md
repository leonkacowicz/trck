# update: add a minimum-size sanity check on the downloaded engine

## Summary
`trck update` validates the download via `compile()` and an `__version__` presence check; add a minimum-byte-size floor to catch truncated downloads.

## Acceptance criteria
- [ ] reject downloads below a sane byte floor before replacing the file

## Notes
