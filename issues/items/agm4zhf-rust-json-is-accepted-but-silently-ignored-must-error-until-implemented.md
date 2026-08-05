# rust: --json is accepted but silently ignored — must error until implemented

## Summary
The Rust argument tables list `--json` as a valid flag for `list`, `tree`, `show`, `ready`,
`next` and `deps` (`crates/trck/src/cli.rs`), but nothing honoured it: the verb printed its
ordinary human output and exited 0. A caller doing `trck list --json | jq` therefore got human
text, a zero exit, and a parse error downstream — the failure surfaced far from its cause.

That was worse than not supporting the flag at all. Rust already rejects genuinely unknown flags
(`list --totally-bogus-flag` → exit 2), so `--json` was uniquely dangerous: the one flag that
lied about having worked.

## Acceptance criteria
- [x] `--json` either produces JSON or fails loudly; it never returns human output with exit 0.
- [x] Until #gh363h3 lands, the flag exits non-zero with a message naming it as not yet
      implemented in this engine — `list: --json is not implemented in this engine yet`, exit 2,
      on all six verbs.
- [x] The behaviour is pinned by a test — see the note below on *where*.

## Notes
Fixed in `usage_error`, ahead of the unrecognized-argument check so the message can name the flag
specifically rather than reporting it as unknown.

**The pinning test is a Rust unit test (`cli::tests::json_is_refused_rather_than_silently_ignored`),
not a conformance fixture, and deliberately so.** Conformance is the *shared* spec: every fixture
asserts one golden that both engines must produce. Here the engines differ on purpose — Python
emits real JSON, Rust refuses — so a fixture would have to fail for one of them by construction.
The conformance fixture for `--json` is the one that pins the *real*, shared behaviour, and it
arrives with #gh363h3 / #t84am5s; this error is transitional scaffolding that gets deleted then.
