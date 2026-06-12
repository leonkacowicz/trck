# `changelog` verb — design

**Date:** 2026-06-12
**Status:** approved for implementation
**Sub-project:** 3 of 3

## Why

Sub-projects 1 and 2 made every issue's `closed` field a precise UTC timestamp
(stamped going forward, backfilled historically). This sub-project delivers the
original feature request: a verb that lists what shipped since a given point, so
a release's timestamp becomes a `--since` filter for generating release notes /
a changelog.

## Scope

In scope:

- A new engine verb `trck changelog --since <DATE|TIMESTAMP>` that prints, as
  nested markdown, the issues completed since the cutoff.
- Selection, hierarchy nesting, sorting, header/count, and error handling.
- Tests.

Out of scope:

- `--json` output (belongs to the in-flight JSON-output epic #024 / #060–063;
  the shared `emit_json` seam does not exist yet).
- `--until` / date ranges, author/component filters, grouping by component or
  kind (the chosen output is a single nested list).
- Any change to how timestamps are stamped or stored (done in sub-projects 1–2).

## Design

### Verb & invocation

```
trck changelog --since <DATE|TIMESTAMP>
```

- New subparser `changelog` in the argparse band; handler `cmd_changelog` in the
  command-handlers band.
- `--since` is **required**. It accepts a bare date (`2026-06-10`) or a full
  timestamp (`2026-06-10T14:00:00Z`). It is lightly validated against the shape
  `^\d{4}-\d{2}-\d{2}(T\d{2}:\d{2}:\d{2}Z)?$`; anything else is a clean
  nonzero-exit error (`die`). No default.
- Reads the index via `build_ctx_or_die` + `load_index`, like other read verbs.

### Selection — the "shipped" set S

An issue is included when **all** hold:

1. its status is terminal — `is_terminal(ctx.cfg, row.status)`;
2. `row.closed` is present **and** `row.closed >= since` (plain Python string
   comparison — both are zero-padded ISO, so this orders correctly; a bare-date
   `since` means "from 00:00 of that day"; the boundary is inclusive, `>=`);
3. `row.resolution` is falsy — excludes `wontfix` / `duplicate` / `superseded`
   (issues closed without shipping anything).

**All kinds are included** (epics, tasks, bugs) — no `kind` filter.

A legacy day-only `closed` (if a tracker was never backfilled) still compares
correctly: `"2026-06-10"` sorts before any same-day timestamp, so it is included
by a `--since 2026-06-10` and excluded by a later-that-day timestamp cutoff,
which is the intuitive result.

### Output — nested markdown to stdout

Build a `Graph(ctx.cfg, S)` over **only the selected rows**. Then:

- **Roots** are members of S whose `parent` is `None` or whose parent is **not**
  in S (`graph.get(parent) is None`). A shipped child whose parent is outside the
  window therefore renders at the top level (orphan → root).
- **Children** of a node are `graph.children_of(node)` (already restricted to S
  because the graph was built from S).
- Render depth-first as indented markdown bullets — two spaces per depth level:
  `"  " * depth + f"- #{id:03d} {title}"`, plus ` ({component})` when
  `row.component` is set (omitted otherwise).
- **Sibling order** (roots and each child list): `closed` **descending**, id
  ascending on ties. Implemented as a stable two-pass sort: sort by `id` then
  sort by `closed` reversed (Python's stable sort preserves the id order within
  equal `closed`).
- **Cycle guard:** a `seen` set passed through the recursion (mirroring
  `forest_layout`) so malformed self/loop parentage can't infinitely recurse.

Header line, then a blank line, then the list:

```
## Shipped since 2026-06-10 — 9 issues

- #002 Part D: richer tracker features (planning)
  - #045 Eliminate unnecessary index double-reads (engine)
  - #043 Extract config-vocabulary validation helpers (engine)
- #058 deps: shorter edges / fewer crossings in the graph layout (deps)
- #047 deps --graph <id>: scope to the focal issue's directed dependency line (deps)
```

- The count in the header is `len(S)` — **all** selected issues, regardless of
  nesting depth.
- Empty result: the header reads `— 0 issues`, followed by a `_none_` line
  (matching the empty-section convention used elsewhere, e.g. SUMMARY).
- The `--since` value is echoed verbatim in the header as the user typed it.

### Components / boundaries

- **`parse_since(value) -> str`** — validate the `--since` shape, `die` on bad
  input, return the value unchanged. Pure (no I/O).
- **`select_shipped(ctx, rows, since) -> list[Issue]`** — the predicate filter
  (terminal + `closed >= since` + no resolution). Pure given rows.
- **`render_changelog(cfg, shipped, since) -> str`** — build the S-graph, walk
  the forest, return the full markdown string (header + body). Pure.
- **`cmd_changelog(args)`** — wire `build_ctx_or_die` → `load_index` →
  `parse_since` → `select_shipped` → `render_changelog`, then `print`.

Splitting selection and rendering keeps each unit independently testable without
git or stdout capture.

### Error handling

- Missing `--since` → argparse's own "required" error (nonzero exit).
- Malformed `--since` → `die("…")` with a clear message and nonzero exit.
- No tracker found → handled by `build_ctx_or_die` (existing behavior).
- An issue whose `parent` points at a non-S issue is treated as a root (not an
  error). Cycles are guarded, not fatal (`check` already forbids them).

## Testing

Tests live in `tests/test_changelog.py`, run under the existing suite. They build
trackers with `make_tracker` + direct `index.jsonl`/file writes (as
`tests/test_timestamps.py` does) and call `cmd_changelog` with captured stdout, or
call the pure helpers directly.

- **Selection:** includes a terminal issue with `closed >= since`; excludes one
  closed **before** `since`; excludes a non-terminal (still-open) issue; excludes
  a terminal issue with `resolution = "wontfix"`; **includes** a `kind = "epic"`
  and a `kind = "bug"`.
- **Boundary:** a bare-date `--since 2026-06-10` includes an issue closed
  `2026-06-10T08:00:00Z`; an exact-timestamp `--since` equal to an issue's
  `closed` includes it (inclusive); a legacy day-only `closed` is handled.
- **Nesting:** a shipped child nests one level under its shipped parent; a shipped
  child whose parent is **not** in S renders at the top level; grandchildren nest
  two levels.
- **Sort:** roots and siblings appear `closed`-descending (newest first), id on
  ties.
- **Header/count:** the count equals the total selected issues (including nested
  ones); the `--since` value appears in the header; component is shown when
  present and omitted when absent.
- **Empty:** no matches → header `— 0 issues` and a `_none_` line.
- **Errors:** malformed `--since` (e.g. `"june"`, `"2026/06/10"`) → `SystemExit`.

## Acceptance criteria

- [ ] `trck changelog --since <DATE|TIMESTAMP>` exists (subparser + `cmd_changelog`).
- [ ] `--since` is required and validated; malformed values exit cleanly.
- [ ] Selection = terminal status + `closed >= since` + no resolution; all kinds included.
- [ ] Output is nested markdown: children under shipped parents, orphans at top level, two-space indent, `- #NNN Title (component)`.
- [ ] Siblings sorted `closed`-descending, id on ties.
- [ ] Header `## Shipped since <since> — N issues` with `N = len(S)`; empty → `— 0 issues` + `_none_`.
- [ ] No `--json` (deferred to #024).
- [ ] Tests cover selection, boundary, nesting/orphans, sort, header/count, empty, and malformed-since; full suite passes.
