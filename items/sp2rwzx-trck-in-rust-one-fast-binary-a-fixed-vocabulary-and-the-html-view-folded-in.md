# trck in Rust: one fast binary, a fixed vocabulary, and the HTML view folded in

## Summary
Replace the generated single-file Python engine with a Rust binary installed on the system,
drop vendoring, fix the vocabulary that is currently configurable, and fold `tools/trck-html`
into the same executable.

**Why, measured.** `trck version` — which does nothing — takes 113ms against a 12ms interpreter
floor: 29ms parsing, 52ms importing the stdlib, 2ms running. 97% of a typical invocation is
getting ready, and the parse share grows linearly with the file because a script gets no
bytecode cache (`__main__` is never cached, by design). Every escape inside Python was measured
and none is worth it: a zipapp of source is *slower* (146ms), a zipapp of `.pyc` (87ms) pins to
one CPython minor version, Cython lands at 82ms for a 2.3MB per-platform binary — within noise
of simply vendoring a directory (82ms), which is free. PyPy is the wrong shape entirely; its
startup is worse and there is no hot loop to JIT. Every compiler option costs exactly what Rust
costs — per-platform artifacts — while buying what bytecode caching gives away. There is no
middle rung: stay interpreted at ~52ms after lazy imports, or leave and get 1–3ms.

**Vendoring was doing one thing worth keeping**, and it was not convenience: it pinned the
format writer to the repo. Nothing else does — `trck.json` has no format field. So versioning
the format is a prerequisite for un-vendoring, in any language.

**Folding the HTML view in is only sane because of Rust.** 86% of `tools/trck-html` is static
assets — 47KB of JS, 13KB of CSS, 1.5KB of shell — that move across as `include_str!` untouched.
Only ~9.6KB is real logic, most of it building the JSON data island the engine needs anyway. In
the Python single file that fold would have been +72KB on 204KB and parse cost on every
invocation; in a Rust binary it is noise. It also removes the runtime coupling that made
`--json` a prerequisite: no accessory imports the engine any more.

## Plan
Four phases. Phase A lands in Python, because it is cheap to iterate there and it shrinks what
Rust has to implement — 58 call sites read the configurable vocabulary today.

- **A. Prepare** (`format`, `vocabulary`, `repo migrate`) — version the config, fix priorities,
  statuses and resolutions to canonical sets with names as display aliases.
- **B. Specify** (`conformance`) — convert the contract tests into language-agnostic fixtures
  with a runner per language. This is what makes the port verifiable rather than hoped about,
  and it doubles as the differential-oracle harness: run both engines, diff the artifacts.
- **C. Port** (`rust`) — build against the fixture suite, red from day one, green as it lands.
- **D. Ship** (`rust: release`, `cutover`) — per-platform artifacts, then un-vendor and retire
  the Python engine.

## What counts as contract
The line is *"would a user or a downstream tool notice if this changed?"* — not "does it go
through a command". That includes the deps gutter strings, row order, `dependency_line` scoping,
demand ranking, readiness, the `needs`/`blocks`/`↑` annotations, and `Issue.to_canonical`, whose
byte-level output the git merge drivers depend on. Many of those are *written* against internal
APIs today for convenience; they still specify observable behaviour and still convert.

What does not convert is the small residue with no observable trace — and most of what looks
like that (`isotonic`, `crossings`, `refine`, `layoutComponent`) is JavaScript inside
`_APP_JS`, which the port does not rewrite at all.

## Notes
- Reshapes two open issues rather than duplicating them: `s3d6xyz` (reconfigure verb to
  rename/reorder statuses) becomes rename-only via display aliases, reorder being gone; and
  `cbf4sxp` (friendly errors for empty vocabulary lists) is moot for statuses and priorities
  once they are fixed. Neither is closed here — decide when `qgpk65t` lands.
- `r9zefup` (`--json`) drops from prerequisite to ordinary work, and gets cheaper: the HTML data
  island needs the same serialiser, so the CLI flag is nearly free on top.
- The JS keeps its own test story. `tests/test_html.py` lifts pure functions out of `_APP_JS`
  and runs them under `node`; the Rust suite needs the same trick, and it is the one part of the
  suite that is not fixtures.
- Release cadence couples once the HTML view is folded in: a graph-layout fix then needs an
  engine release.
