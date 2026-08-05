# rust: fold in the html view

## Summary
`tools/trck-html` becomes a subcommand. 86% of it is static assets that move across untouched —
47KB of JS, 13KB of CSS, 1.5KB of shell — leaving ~9.6KB of real logic, most of which is
building the JSON data island.

## Acceptance criteria
- [x] `_APP_JS`, `_CSS` and `_SHELL` embedded verbatim as compile-time assets.
- [x] The data island built from the Rust model, matching the current schema field for field —
      the JS reads it and is not being rewritten.
- [x] Output self-contained: no network, no external refs. The existing assertion carries over.
- [x] The JS keeps its test story: the Rust suite lifts pure functions out of the asset and runs
      them under `node`, as `tests/test_html.py` does now. This is the one part of the suite that
      is not fixtures.
- [x] `tools/trck-html` deleted, not left as a second implementation.

## Landed
`f17aa35`. `tools/trck-html` is gone; `trck html` replaces it.

**The generated page is byte-identical** to the Python tool's — 605,570 bytes for this repo,
98,397 for the example. Ported first, compared, and only then deleted: the oracle has to
outlive the thing it verifies. Re-confirmed after deletion by stashing the removal and
regenerating.

**One byte apart at first**, which is exactly what this comparison is for. `_APP_JS` opened
with `r"""` and a newline, so the embedded string began with one, while `_CSS` and `_SHELL`
used `"""\` and did not. The asset keeps that leading newline rather than the renderer
growing a magic `\n` — the assets are supposed to move across untouched, and that stays
literally true.

**The self-containment assertion is sharper than the one it replaces.** It tests what gets
*fetched*, not what merely contains a URL: `SVGNS` is `http://www.w3.org/2000/svg`, an XML
namespace identifier `createElementNS` requires and nothing ever requests. A blanket no-http
rule fails on it and teaches the next reader to weaken the check.

`html` has no Python counterpart, so it is the first verb where the two engines deliberately
differ. The verb list is what the binary offers, not a mirror of the old CLI.
