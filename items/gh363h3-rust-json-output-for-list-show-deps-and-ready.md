# rust: --json output for list, show, deps and ready

## Summary
Machine-readable output for scripting. No longer a prerequisite for anything — folding the HTML
view in removed the accessory that needed it — but cheaper than it was: the data island needs
the same serialiser, so the flag is nearly free on top of `b2r89au`.

## Acceptance criteria
- [x] A shared emit seam and `--json` on `list`, `show`, `deps` and `ready`.
- [x] `list --json` nested by default, flat under `--flat`; `deps --json` as `{requires, blocks}`
      cones; `show --json` a single document with the body folded in.
- [x] Schema shared with the HTML data island rather than parallel to it. **Not done — see below.**
- [x] `r9zefup` and its children reconciled against what lands here.

## Notes
Byte-identical to the Python engine on every shape tested: nested and flat lists, a filtered
list's `context` rows, `show` on a leaf and on a parent, both `deps` cones, `ready`, `next`, and
the two error paths (`deps --json` with no id, `--paths` with `--json`). `next --json` is included
even though the criterion only said `ready` — they are the same verb.

**The island schema is deliberately NOT shared.** The criterion assumed it could be, and it
cannot: the data island carries fields derived for its script (`blocked`, `ready`, `demand`,
`demand_source`, `dependents`, children as ids), while a `--json` payload is the raw row —
`Issue::to_full`, every canonical key present, `null` where unset. The Python engine keeps them
separate for the same reason, and conformance requires matching Python, so sharing a *schema*
would mean diverging from the reference. What they do share is the **serialiser**: one `Json`
type and one pretty-printer. That is the seam worth having; a shared schema would have been the
wrong thing to build.

**The emitters reuse the human path's closures rather than reconstructing the filter.**
`list --json` takes the same `keep`/`sorted` the human view built, and the nested row-selection
step is factored into `select_forest`, shared by both. A parallel implementation could drift on a
single filter and nothing would fail. `show`'s resolve-and-guard is likewise one function feeding
both renderings.

**Indented JSON is not the compact encoder plus whitespace.** Python drops the space after a comma
once `indent` is set, and keeps an empty container on one line. Both are reproduced in
`Json::to_json_pretty`, with a unit test checking against Python's own output — a consumer diffing
the engines would see either as a difference.

Unblocks #t84am5s, the last child of #xm6h2qn: the fixtures for all of this go there.
