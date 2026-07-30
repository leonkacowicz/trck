# docs: refresh the shipped trck docs — flat layout, repo setup-git/migrate-layout

## Summary

`CLAUDE_MD_TEMPLATE` in `src/trck/templates.py` — the contributor guide `trck init` writes to
`<tracker>/CLAUDE.md`, and that `trck update` refreshes in place — trails three landed changes.
It's the doc a consumer repo's agents and contributors read, so the gaps bite exactly the audience
that has no other source.

1. **Commit hygiene claims a file moves.** "The moved file, `index.jsonl`, and `SUMMARY.md` are one
   tracker change" predates the flat layout (#2srvf6j): a status change now rewrites the index and
   `SUMMARY.md` and never touches the body file.
2. **Half the `trck repo` group is missing.** The list stops at `normalize` / `renumber` /
   `install-hook`; `setup-git`, `merge-index`, `merge-summary` (#ey2aruc — that epic closed with no
   docs child, which is why they never landed here) and `migrate-layout` are absent. `setup-git` is
   the costly omission: it's needed once per clone, and a contributor who skips it just gets raw
   conflict markers in `index.jsonl` with nothing explaining why.
3. **The legacy-layout refusal is undocumented.** A pre-0.23 tracker fails every verb with
   `legacy status-folder layout: …`; the doc never names `trck repo migrate-layout` as the fix.

The root `CLAUDE.md` is already current on all three.

`skills/trck/SKILL.md` — the agent skill this repo ships — carried the same three gaps plus two
more: it still listed `normalize` / `renumber` /
`install-hook` as *top-level* verbs (they moved under `trck repo` in 41831aa), and described
priority as inert, predating demand-cone ranking (#9bktptp).

## Acceptance criteria
- [x] Commit-hygiene paragraph no longer implies the body file moves on a status change
- [x] Verb list covers `setup-git` (with the once-per-clone rationale), `merge-index`,
      `merge-summary`, and `migrate-layout`
- [x] The legacy status-folder refusal and its fix are documented
- [x] `issues/CLAUDE.md` (this repo's already-scaffolded copy, which init wrote and build.py does
      not regenerate) carries the same edits
- [x] `skills/trck/SKILL.md`: flat layout + legacy refusal documented, maintenance verbs regrouped
      under `trck repo`, demand ranking and `ready ID` subtree scoping described
- [x] `python3 build.py --check` and `./trck check` pass; full suite green

## Notes
- No behavioural test: the change is template prose. `tests/test_update.py` exercises the
  refresh *mechanism* against a synthetic `CLAUDE_MD_TEMPLATE` literal, so it stays green and
  keeps covering the path that ships this text to consumer repos.
