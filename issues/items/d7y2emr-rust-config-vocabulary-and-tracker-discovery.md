# rust: config, vocabulary and tracker discovery

## Summary
Loading `trck.json`, the fixed vocabulary, the format guard, and walking up for the tracker
directory. Much smaller than its Python counterpart, because phase A removed the configurable
vocabulary the 58 call sites were reading.

## Acceptance criteria
- [ ] Discovery by walking up for `trck.json`, with `--dir` and `$TRCK_DIR` overrides.
- [ ] Config load, merge over defaults, and the format/extensions guard from `9fajv3x`.
- [ ] Display aliases resolved on input and applied on output, canonical values on disk.
- [ ] Config errors name the file and the key.
