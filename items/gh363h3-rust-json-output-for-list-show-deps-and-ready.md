# rust: --json output for list, show, deps and ready

## Summary
Machine-readable output for scripting. No longer a prerequisite for anything — folding the HTML
view in removed the accessory that needed it — but cheaper than it was: the data island needs
the same serialiser, so the flag is nearly free on top of `b2r89au`.

## Acceptance criteria
- [ ] A shared emit seam and `--json` on `list`, `show`, `deps` and `ready`.
- [ ] `list --json` nested by default, flat under `--flat`; `deps --json` as `{requires, blocks}`
      cones; `show --json` a single document with the body folded in.
- [ ] Schema shared with the HTML data island rather than parallel to it.
- [ ] `r9zefup` and its children reconciled against what lands here.
