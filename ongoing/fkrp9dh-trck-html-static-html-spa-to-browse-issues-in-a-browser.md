# trck-html: static HTML SPA to browse issues in a browser

## Summary

A standalone accessory tool, **`tools/trck-html`**, that reads a tracker and generates a
**single self-contained HTML file** — a client-rendered single-page app for browsing issues
in a browser, no server and no network required to open it.

It is deliberately **not** part of the amalgamated engine: it is a separate executable that
loads the generated `./trck` as a module at runtime and reuses the engine's own API, so no
tracker logic is duplicated. The static file is **read-only** (there is no process to run
verbs); richer, write-capable grooming is a later `trck serve` phase.

Built **minimum-viable first, then incremented** — see the roadmap below. This epic tracks
the whole arc; each phase is a child issue small enough to finish in one go.

## Architecture

- **`tools/trck-html`** — extensionless executable (`#!/usr/bin/env python3`, `chmod +x`),
  standard-library only, run as `./tools/trck-html`.
- **Engine reuse:** at runtime it loads the repo's generated `./trck` via
  `importlib.machinery.SourceFileLoader` (exactly as `tests/helpers.py::load_trck` does),
  then reuses:
  - discovery — `find_dir` / `build_ctx` (same tracker resolution as `./trck`; walks up for
    `trck.json`),
  - data — `load_graph(ctx)` → the `Graph` (children, requires/dependents, blocked/ready,
    `leaf_rollup` for progress %),
  - vocabulary — config helpers (`status_names` + initial/terminal roles, `priority_rank`).
    Statuses/priorities/kinds are **read from config, never hardcoded.**
- **Pure core `render_html(ctx) -> str`** — the reusable heart. Produces the whole document:
  a **JSON data island** (`<script type="application/json" id="trck-data">…</script>`,
  with `<` escaped to `<` so issue bodies can never break out of the script tag or the
  JSON), plus **inline `<style>`** and an **inline vanilla-JS app** that renders everything
  client-side. A future `trck serve` / in-engine `trck html` subcommand reuses this same
  function (serve could also expose the island as an API).

## CLI surface

```
./tools/trck-html            # discover tracker → write issues/issues.html
./tools/trck-html -o out.html
./tools/trck-html -o -       # write to stdout
./tools/trck-html PATH       # optional: point at a specific tracker/repo
```

## Data model (the JSON island)

Per issue: `id`, `title`, `status`, `priority`, `kind`, `labels`, `resolution`, `parent`,
`children[]`, `requires[]`, `dependents[]` (blocks), `points`, `progress` %, `blocked`,
`ready`, and raw `body` text. Plus a `config` block (status names + roles, priority order)
and the `repo` name. The client renders entirely from this; `body` ships raw and is rendered
per the body-rendering phase in effect.

## Roadmap (build order)

Each row is (or becomes) a child issue of this epic. Sub-tasks are unordered by nesting;
where one truly must precede another, wire an explicit dependency.

| Phase | Scope |
|---|---|
| **v1 — MVP** | Script + `render_html` + JSON island + SPA shell. Filterable/searchable **issue list**; click an issue → **detail panel** (metadata, clickable deps/parent/children, body as HTML-escaped `<pre>`). |
| v2 | **Command-copy**: stage status/priority changes in the UI → show the equivalent `./trck …` commands to copy-paste. No persistence. |
| v3 | **Dependency graph** view (clickable; nodes focus the issue). |
| v4 | **Tree / hierarchy** view with rolled-up progress %. |
| v5 | **Board / kanban** by status. |
| v6 | Body **markdown**: JS subset renderer (headings, `- [ ]`, lists, code, links) → optional JS lib, with the escaped `<pre>` as the guaranteed fallback floor. |
| v7 | **`trck serve`**: live process; edits write back to `index.jsonl`. |

## Acceptance criteria

This epic is done when its child phases are. For **v1 (MVP)** specifically:

- [ ] `tools/trck-html` exists, is executable, stdlib-only, and loads `./trck` at runtime.
- [ ] Discovers the tracker like `./trck`; writes `issues/issues.html` by default; `-o PATH`
      overrides; `-o -` writes to stdout.
- [ ] `render_html(ctx)` returns one self-contained HTML string (inline CSS + JS + JSON
      island); opening it needs no network.
- [ ] The JSON island contains every issue with the fields above; bodies are escaped so
      `</script>`, `<`, `>`, `&` cannot break out.
- [ ] SPA renders a filterable/searchable list and a detail panel with clickable deps.
- [ ] Tests build a temp tracker, load the script, and assert on `render_html` output
      (issues present, correct fields, escaping, shell structure).

## Notes

**Accepted trade-offs (recorded deliberately):**
- The accessory rides trck's **internal** API (`load_graph`, `Graph`, config helpers), not a
  public surface — it is coupled to those signatures and may need touch-ups if they change.
  It lives outside the amalgamation, so it can't drift into `./trck`, but it is **not** covered
  by `build.py --check` either. It gets its own tests under `tests/`.
- The static file is **read-only**; command-copy (v2) is the closest it gets to editing until
  `trck serve` (v7).

**Testing seam:** because `render_html(ctx) -> str` is pure, v1 is fully unit-testable from
Python without a browser (assert on the returned string / JSON island). The JS rendering
itself is verified by opening the file — an accepted manual-check limitation for an accessory
tool.

**Related work (not dependencies):**
- `#v9nyy42` Part C: curses TUI for browsing — a different read-only browser (terminal, not
  HTML); shares the "read-only browse" goal but no code.
- `#r9zefup` Add `--json` output to list/show/deps/tree — parallel serialization effort, but
  trck-html reuses the internal `Graph` directly rather than the CLI's `--json`, so there is no
  ordering dependency.

**Design origin:** this body *is* the design spec (per request, kept in the epic rather than a
separate `docs/specs/` file).
