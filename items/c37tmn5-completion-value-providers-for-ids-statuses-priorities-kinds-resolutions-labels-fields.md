# completion: value providers for ids, statuses, priorities, kinds, resolutions, labels, fields

## Summary
The candidate lists themselves: given a resolved tracker, return the values of each kind, each
paired with a short description for the shells that can show one (zsh `_describe`, fish's
tab-separated form). Ids are the important case — a 7-char random id is meaningless on its own,
so the issue title is the description that makes completion usable at all.

Sources are all existing: statuses/priorities/kinds/resolutions from `trck.json` via the config
helpers, ids/titles/labels/custom-field names from `index.jsonl`. Nothing here should re-read or
re-parse anything the engine already knows how to load.

## Acceptance criteria
- [ ] A provider per kind: ids (described by title), statuses, priorities, kinds, resolutions,
      labels (the union across issues), custom field names.
- [ ] Values come from `load_config`/the index — no literal status or priority names.
- [ ] Descriptions are single-line and stripped of anything that would break the shell's
      value/description separator.
- [ ] Ordering is deterministic and useful: config order for vocabulary, and for ids something
      better than random (most recently touched, or index order) — decide and note the choice.
- [ ] Tests cover each provider against a fixture tracker, including a reconfigured vocabulary.

## Notes
- Open question worth settling here rather than in the callback: whether id completion should
  offer *all* ids or filter by context (`start <TAB>` arguably wants non-terminal issues only,
  `done <TAB>` wants in-flight ones). Filtering is friendlier but surprising when you genuinely
  need a done issue; the safe default is everything, ranked.
- Blocks [[qhf5fa2]]. Part of [[9echsrh]].
