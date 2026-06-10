# Sprite atlas build tool

## Summary
Build-time tool that packs individual sprite PNGs into a single texture atlas plus a JSON
manifest of frame rects, so the runtime does one texture bind.

## Acceptance criteria
- [x] Packs a folder of PNGs into one atlas + manifest
- [x] Manifest maps sprite name → frame rect and pivot
- [x] Re-runs incrementally when only some inputs change

## Notes
Shipped. Upstream of every other art task (#017–#020) — the fan-out you see in `deps
--graph`.
