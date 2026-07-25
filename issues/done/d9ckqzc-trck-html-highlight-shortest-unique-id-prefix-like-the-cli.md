# trck-html: highlight shortest-unique id prefix (like the CLI)

## Summary
The CLI bolds each issue id's shortest unique prefix (git-short-hash style, via
`unique_prefix_lens` + `hl_id`) so you can see the fewest chars you'd type to
resolve it. The HTML browser renders every id flat as `#<full id>`. Mirror the CLI:
compute each id's shortest-unique-prefix length in the model and render the prefix
emphasised, the remainder dimmed, everywhere an id is shown as an identifier
(list rows, tree rows, board cards, detail heading, graph nodes).

## Acceptance criteria
- [x] Model carries each issue's shortest-unique-prefix length (`plen`), computed with
      the engine's `unique_prefix_lens` so it matches the CLI exactly.
- [x] List, tree, board, and detail-heading ids render the prefix emphasised and the
      remainder dimmed.
- [x] Graph SVG node labels highlight the prefix too (via tspans).
- [x] Tests cover `plen` (cross-checked against `unique_prefix_lens`) and the presence
      of the highlight styling/markup.

## Notes
- Inline dependency links (`.ilink`, `#id title`) are intentionally left as plain accent
  links — they're titled navigation, not bare identifiers, so the prefix highlight would
  fight the link colour.
