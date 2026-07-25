# v6: markdown bodies — JS subset renderer, then optional lib, <pre> fallback

## Summary

Render issue bodies as **markdown** in the detail panel instead of an escaped `<pre>`.
A small, self-contained JS subset renderer covers trck's Summary / Acceptance / Notes
vocabulary. The escaped `<pre>` stays as the guaranteed floor: a **raw ⇄ rendered** toggle
plus a try/catch fallback.

## Design

**Decision — no external markdown lib.** A CDN/bundled lib can't load from `file://` or
offline and cuts against the no-network ethos, so the "optional lib" arm of the roadmap is
dropped in favour of the self-contained subset renderer.

**Safety (the important part):** the renderer builds DOM via `createElement`/`textContent`,
**never `innerHTML`**, so body text can't inject executable HTML. Link URLs are
scheme-validated (`http/https/mailto/#/relative` only; `javascript:`/`data:` rejected). The
escaped `<pre>` remains the floor if rendering ever throws.

**Subset supported:** ATX headings (`#`…`######`), paragraphs, unordered/ordered lists,
task items `- [ ] / - [x]` (read-only checkboxes), fenced ``` code blocks, inline
`` `code` ``, `**bold**`, `*italic*`/`_italic_`, and `[text](url)` links. HTML comments
(`<!-- … -->`, the template hints) are stripped from the rendered view. Anything unsupported
falls through as paragraph text.

**View (JS; browser-verified):** the detail body becomes a container with a small
`raw`/`rendered` toggle (defaults to rendered, mode sticky across issues via state). Rendered
markdown gets `.body.md` styling; raw shows the existing escaped `<pre>`.

## Acceptance criteria

- [ ] Bodies render as markdown by default in the detail panel; a toggle switches to raw `<pre>`.
- [ ] Rendering is DOM-based (no `innerHTML`); link schemes validated; `<pre>` fallback on error.
- [ ] The raw body is still shipped intact in the JSON island (unchanged) and stays escaped.
- [ ] v1–v5 behaviour unchanged; full suite + `build.py --check` green.

## Notes

The renderer is pure client-side JS, so its correctness (formatting + XSS-inertness) is
browser-verified; the Python tests cover that the raw body is still shipped/escaped and that
the render/raw toggle UI is present. This is a deliberate limitation of the accessory's
JS-not-unit-tested architecture — mitigated by the DOM-only, scheme-checked implementation
and the escaped-`<pre>` floor. Parent: epic #fkrp9dh.
