# v2: command-copy — stage status/priority edits, show equivalent ./trck commands

## Summary

The static file is read-only (no process to run verbs). This phase adds the closest
thing to editing: in the detail panel the user can **stage** status/priority changes,
and the app surfaces the equivalent **`trck` commands to copy-paste** into a terminal.
Nothing persists — it just generates the commands. (Real write-back is v7 `trck serve`.)

## Design

**Python seam (testable):**
- New `--cmd PREFIX` CLI flag, embedded as `config.cmd` in the JSON island; the JS
  prepends it to every generated command.
- Default is auto-derived: `trck` if a global `trck` is on `PATH`, else a repo-relative
  path to the loaded engine (`./trck`, or `./issues/trck` when vendored). `--cmd`
  overrides.

**SPA (JS; verified in-browser):**
- Detail panel gains a small **"Stage a change"** area: a status `<select>` and a
  priority `<select>` pre-set to the issue's current values.
- Changing a control **stages** a pending edit for that (issue, field); setting it back
  to the original value clears that edit.
- A **"Pending changes"** tray (docked footer, shown only when non-empty) lists the
  generated commands with **Copy all** and **Clear** buttons. Edited issues show a marker
  in the list/detail.
- Command mapping (vocabulary-agnostic, always correct):
  - status → `{cmd} mv {id} {status}`
  - priority → `{cmd} set {id} --priority {priority}`

## Acceptance criteria

- [ ] `--cmd PREFIX` flag; `config.cmd` present in the island, honoured by the JS.
- [ ] Default `cmd` = `trck` when on PATH, else a repo-relative engine path.
- [ ] Detail panel renders status + priority editors seeded to current values.
- [ ] Staging an edit adds the right command to the pending tray; reverting removes it.
- [ ] Copy-all copies every pending command; Clear empties the tray.
- [ ] Bodies/other v1 behaviour unchanged; full suite + `build.py --check` green.

## Notes

The command-*building* logic runs client-side (JS), so the Python tests cover the
seam — the `config.cmd` default/override and that the staging UI hooks are present in the
rendered document — while the interactive behaviour is verified by opening the file
(same manual-check limitation as v1). Parent of this issue: epic #fkrp9dh.
