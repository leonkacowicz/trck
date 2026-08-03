# rust: fold in the html view

## Summary
`tools/trck-html` becomes a subcommand. 86% of it is static assets that move across untouched —
47KB of JS, 13KB of CSS, 1.5KB of shell — leaving ~9.6KB of real logic, most of which is
building the JSON data island.

## Acceptance criteria
- [ ] `_APP_JS`, `_CSS` and `_SHELL` embedded verbatim as compile-time assets.
- [ ] The data island built from the Rust model, matching the current schema field for field —
      the JS reads it and is not being rewritten.
- [ ] Output self-contained: no network, no external refs. The existing assertion carries over.
- [ ] The JS keeps its test story: the Rust suite lifts pure functions out of the asset and runs
      them under `node`, as `tests/test_html.py` does now. This is the one part of the suite that
      is not fixtures.
- [ ] `tools/trck-html` deleted, not left as a second implementation.
