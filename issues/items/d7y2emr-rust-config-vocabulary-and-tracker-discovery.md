# rust: config, vocabulary and tracker discovery

## Summary
Loading `trck.json`, the fixed vocabulary, the format guard, and walking up for the tracker
directory. Much smaller than its Python counterpart, because phase A removed the configurable
vocabulary the 58 call sites were reading.

## Acceptance criteria
- [x] Discovery by walking up for `trck.json`, with `--dir` and `$TRCK_DIR` overrides.
- [x] Config load, merge over defaults, and the format/extensions guard from `9fajv3x`.
- [x] ~~Display aliases resolved on input and applied on output, canonical values on
      disk.~~ **Struck.** Display aliases were the two-vocabulary design — canonical
      states with per-tracker names over them — and `qgpk65t` deleted it. There is one
      vocabulary, so what is on disk *is* what is displayed and there is nothing to
      resolve.
- [x] Config errors name the file and the key.

## Landed
`config.rs` and `discovery.rs`, 26 new tests. Loads this repo's and the example's real
`trck.json` as a permanent test, so a guard that rejects a live tracker fails here.

**One known divergence, recorded rather than papered over.** Python reports a malformed
config as `{path}: invalid JSON ({e})` where `{e}` is `json.JSONDecodeError`'s own text
("Expecting value: line 1 column 1 (char 0)"). Rust's parser words its complaint
differently and always will. The prefix matches; the decoder's own phrasing does not, and
should not be treated as contract — a conformance fixture asserting that string would be
pinning CPython's error formatting, not trck's behaviour.

The vocabulary predicates take no config argument, unlike Python's, which still thread a
`cfg` they ignore. That thread is a scar from when it was configurable; there was no
reason to reproduce it here.
