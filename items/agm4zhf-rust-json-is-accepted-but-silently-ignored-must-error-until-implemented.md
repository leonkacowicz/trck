# rust: --json is accepted but silently ignored — must error until implemented

## Summary
The Rust argument tables list `--json` as a valid flag for `list`, `show`, `ready` and `next`
(`crates/trck/src/cli.rs`), but nothing honours it: the verb prints its ordinary human output and
exits 0. A caller doing `trck list --json | jq` therefore gets human text, a zero exit, and a
parse error downstream — the failure surfaces far from its cause.

This is worse than not supporting the flag at all. Rust already rejects genuinely unknown flags
(`list --totally-bogus-flag` → `error: list: unrecognized argument …`, exit 2), so `--json` is
uniquely dangerous: it is the one flag that lies about having worked.

## Acceptance criteria
- [ ] `--json` either produces JSON or fails loudly; it never returns human output with exit 0.
- [ ] Until #gh363h3 lands, the flag exits non-zero with a message naming it as not yet
      implemented in this engine.
- [ ] A conformance fixture pins whichever behaviour is chosen.

## Notes
Found while inventorying the command-output surface for #xm6h2qn. #gh363h3 is the issue that
actually implements `--json` for the read verbs; this one is only about not lying in the meantime,
so it should land first and be cheap. If #gh363h3 is picked up straight away, fold this into it
and close this one as superseded.
