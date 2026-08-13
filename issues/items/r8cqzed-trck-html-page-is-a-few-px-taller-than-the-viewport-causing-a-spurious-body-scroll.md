# trck-html: page is a few px taller than the viewport, causing a spurious body scroll

## Summary

The generated HTML view lays out a couple of pixels taller than the browser viewport, so the
whole page gets a tiny vertical scrollbar that scrolls almost nothing. Every pane inside already
scrolls on its own; the outer scroll is pure noise — it steals a scroll gesture, shifts the
sticky filter bar, and makes the app look like it doesn't quite fit.

The height of the app shell is guessed rather than measured. `assets/app.css:54` has:

    main { ...; height: calc(100vh - 92px); }

`92px` is a hardcoded stand-in for the height of `.topbar` + `.filters` (both `border-bottom`ed,
both `flex-wrap: wrap`). Whenever the real chrome is taller than 92px — different font metrics, a
zoom level, or the filter bar wrapping to a second line — `header + main` exceeds `100vh` and the
body scrolls by the difference. (It can also come out *shorter*, leaving a dead gap under `main`.)

The fix is to stop guessing: make the page a viewport-height flex/grid column where `main` takes
the remaining space, so no constant has to match the header. Note `100vh` on mobile browsers also
includes the retracting URL bar; `100dvh` is the correct unit if the viewport unit is kept at all.

## Acceptance criteria
- [ ] At default zoom the page produces no document-level vertical scrollbar:
      `document.documentElement.scrollHeight === document.documentElement.clientHeight`.
- [ ] Still true when the filter bar wraps to a second line (narrow window) and at 80%/125%
      browser zoom — i.e. the layout no longer depends on the chrome being exactly 92px.
- [ ] The left list, detail pane, graph, tree and board panes still scroll internally and still
      fill the window; no dead gap appears below `main`.
- [ ] The `max-width: 720px` single-column breakpoint, which deliberately lets the page scroll
      (`height: auto`), is unaffected.

## Notes

- Only `assets/app.css` should need changing; it is compiled into the binary as a string, so
  rebuild before checking a regenerated page.
- Related in spirit, not a duplicate: #jfur4ky (view pane scroll position resets on selection).
