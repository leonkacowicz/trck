# `changelog` Verb Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add a `trck changelog --since <DATE|TIMESTAMP>` verb that prints, as nested markdown, the issues completed (closed in a terminal status, excluding wontfix/duplicate/superseded) since a cutoff — release notes from the tracker.

**Architecture:** Four functions in the single-file engine `./trck`: `parse_since` (validate the cutoff), `select_shipped` (filter rows), `render_changelog` (build a `Graph` over only the selected rows and walk it into indented markdown), and `cmd_changelog` (wire them + an argparse subparser). All pure except the handler.

**Tech Stack:** Python 3 standard library only. Engine reuses existing helpers: `die`, `build_ctx_or_die`, `load_index`, `is_terminal`, and the `Graph` class. Tests run via `python3 -m unittest discover -s tests`.

**Spec:** `docs/specs/2026-06-12-changelog-verb-design.md`

---

## File Structure

- **`trck`** (engine) — all production changes:
  - New module constant `SINCE_RE` and function `parse_since` (near the other small helpers / just above the new handlers).
  - New functions `select_shipped`, `render_changelog`, `cmd_changelog` placed together in the command-handlers band, immediately **above** `def cmd_version` (line ~1758) — all their dependencies (`die`, `Graph`, `is_terminal`, `build_ctx_or_die`, `load_index`) are defined earlier in the file.
  - New `changelog` subparser registered in `build_parser`, immediately **before** the `ck = sub.add_parser("check", …)` block (~line 2117), with `set_defaults(func=cmd_changelog)`.
- **`tests/test_changelog.py`** (new) — unit tests for the three pure helpers and an end-to-end test through `cmd_changelog`.

Key engine facts to rely on:
- `component` is a **custom field** (not in `CANON_KEYS`), so it is read into `row.extra` — access it as `row.extra.get("component")`, never `row.component`.
- `Graph(cfg, rows)` builds a parent→children map over exactly the `rows` passed; `g.by_id` maps id→Issue for those rows; `g.children_of(r)` returns that row's children (id-sorted) restricted to the passed rows.
- The default test config (`make_tracker(tmp, {})`) has statuses `backlog`/`ongoing`/`done` with `done` terminal.
- `load_index` needs only `index.jsonl` (no body files) to return `Issue` rows.

---

### Task 1: `parse_since` — validate the cutoff

**Files:**
- Modify: `trck` (add `SINCE_RE` + `parse_since`)
- Test: `tests/test_changelog.py` (new)

- [ ] **Step 1: Write the failing test**

Create `tests/test_changelog.py`:

```python
import json
import unittest
from io import StringIO
from contextlib import redirect_stdout
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


def row(iid, *, status="done", closed=None, kind="task", parent=None,
        resolution=None, component=None, title=None):
    d = {"id": iid, "slug": f"i{iid}", "title": title or f"I{iid}", "kind": kind,
         "status": status, "priority": "medium"}
    if parent is not None:
        d["parent"] = parent
    if closed is not None:
        d["closed"] = closed
    if resolution is not None:
        d["resolution"] = resolution
    if component is not None:
        d["component"] = component
    return json.dumps(d, ensure_ascii=False)


class TestParseSince(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_accepts_bare_date(self):
        self.assertEqual(self.t.parse_since("2026-06-10"), "2026-06-10")

    def test_accepts_full_timestamp(self):
        self.assertEqual(self.t.parse_since("2026-06-10T14:00:00Z"), "2026-06-10T14:00:00Z")

    def test_rejects_garbage(self):
        for bad in ("june", "2026/06/10", "2026-6-10", "2026-06-10T14:00Z", ""):
            with self.assertRaises(SystemExit):
                self.t.parse_since(bad)


if __name__ == "__main__":
    unittest.main()
```

- [ ] **Step 2: Run it to verify it fails**

Run: `python3 -m unittest tests.test_changelog.TestParseSince -v`
Expected: FAIL — `AttributeError: module 'trck_engine' has no attribute 'parse_since'`.

- [ ] **Step 3: Implement `SINCE_RE` and `parse_since`**

In `trck`, add this immediately above where the new handlers will go (just before `def cmd_version`, ~line 1758):

```python
SINCE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}(T\d{2}:\d{2}:\d{2}Z)?$")


def parse_since(value: str) -> str:
    """Validate a --since cutoff: a bare date (YYYY-MM-DD) or a full UTC
    timestamp (YYYY-MM-DDTHH:MM:SSZ). Returns it unchanged, or dies."""
    if not SINCE_RE.match(value):
        die(f"--since must be a date (YYYY-MM-DD) or timestamp "
            f"(YYYY-MM-DDTHH:MM:SSZ), got {value!r}")
    return value
```

(`re` and `die` are already imported/defined at the top of the file.)

- [ ] **Step 4: Run the test to verify it passes**

Run: `python3 -m unittest tests.test_changelog.TestParseSince -v`
Expected: PASS (3 tests).

- [ ] **Step 5: Commit**

```bash
git add trck tests/test_changelog.py
git commit -m "changelog: parse_since cutoff validation"
```

---

### Task 2: `select_shipped` — the filter

**Files:**
- Modify: `trck` (add `select_shipped`)
- Test: `tests/test_changelog.py`

- [ ] **Step 1: Write the failing test**

Append to `tests/test_changelog.py`:

```python
class TestSelectShipped(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def load(self, tmp, rows_json):
        d = make_tracker(tmp, {})
        (d / "index.jsonl").write_text("".join(r + "\n" for r in rows_json))
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return ctx, self.t.load_index(ctx)

    def ids(self, ctx, rows, since):
        return sorted(r.id for r in self.t.select_shipped(ctx.cfg, rows, since))

    def test_selection_matrix(self):
        with TemporaryDirectory() as tmp:
            ctx, rows = self.load(tmp, [
                row(1, closed="2026-06-11T10:00:00Z"),                       # in: terminal, after
                row(2, closed="2026-06-09T10:00:00Z"),                       # out: closed before since
                row(3, status="ongoing"),                                    # out: not terminal (no closed)
                row(4, closed="2026-06-11T10:00:00Z", resolution="wontfix"), # out: resolution
                row(5, closed="2026-06-12T10:00:00Z", kind="epic"),          # in: epics included
                row(6, closed="2026-06-12T10:00:00Z", kind="bug"),           # in: bugs included
            ])
            self.assertEqual(self.ids(ctx, rows, "2026-06-10"), [1, 5, 6])

    def test_bare_date_includes_same_day_timestamp(self):
        with TemporaryDirectory() as tmp:
            ctx, rows = self.load(tmp, [row(1, closed="2026-06-10T08:00:00Z")])
            self.assertEqual(self.ids(ctx, rows, "2026-06-10"), [1])

    def test_exact_timestamp_boundary_is_inclusive(self):
        with TemporaryDirectory() as tmp:
            ctx, rows = self.load(tmp, [row(1, closed="2026-06-10T08:00:00Z")])
            self.assertEqual(self.ids(ctx, rows, "2026-06-10T08:00:00Z"), [1])

    def test_legacy_day_only_closed_handled(self):
        with TemporaryDirectory() as tmp:
            ctx, rows = self.load(tmp, [row(1, closed="2026-06-11")])
            self.assertEqual(self.ids(ctx, rows, "2026-06-10"), [1])
            self.assertEqual(self.ids(ctx, rows, "2026-06-12"), [])
```

- [ ] **Step 2: Run to verify it fails**

Run: `python3 -m unittest tests.test_changelog.TestSelectShipped -v`
Expected: FAIL — `AttributeError: module 'trck_engine' has no attribute 'select_shipped'`.

- [ ] **Step 3: Implement `select_shipped`**

In `trck`, add just below `parse_since`:

```python
def select_shipped(cfg: dict, rows: list, since: str) -> list:
    """Issues that 'shipped' on/after `since`: in a terminal status, with a
    `closed` value >= since (plain ISO string compare), and no resolution
    (so wontfix/duplicate/superseded are excluded). All kinds are included."""
    out = []
    for r in rows:
        if not is_terminal(cfg, r.status):
            continue
        if not r.closed or r.closed < since:
            continue
        if r.resolution:
            continue
        out.append(r)
    return out
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `python3 -m unittest tests.test_changelog.TestSelectShipped -v`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add trck tests/test_changelog.py
git commit -m "changelog: select_shipped filter (terminal, closed>=since, no resolution)"
```

---

### Task 3: `render_changelog` — nested markdown

**Files:**
- Modify: `trck` (add `render_changelog`)
- Test: `tests/test_changelog.py`

- [ ] **Step 1: Write the failing test**

Append to `tests/test_changelog.py`:

```python
class TestRenderChangelog(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def load(self, tmp, rows_json):
        d = make_tracker(tmp, {})
        (d / "index.jsonl").write_text("".join(r + "\n" for r in rows_json))
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return ctx, self.t.load_index(ctx)

    def test_header_count_and_flat_lines(self):
        with TemporaryDirectory() as tmp:
            ctx, rows = self.load(tmp, [
                row(1, closed="2026-06-11T10:00:00Z", component="engine", title="Alpha"),
                row(2, closed="2026-06-12T10:00:00Z", component="deps", title="Beta"),
            ])
            shipped = self.t.select_shipped(ctx.cfg, rows, "2026-06-10")
            out = self.t.render_changelog(ctx.cfg, shipped, "2026-06-10")
            self.assertTrue(out.startswith("## Shipped since 2026-06-10 — 2 issues\n\n"))
            # newest closed first: #002 (06-12) before #001 (06-11)
            self.assertLess(out.index("#002 Beta (deps)"), out.index("#001 Alpha (engine)"))

    def test_component_omitted_when_absent(self):
        with TemporaryDirectory() as tmp:
            ctx, rows = self.load(tmp, [row(1, closed="2026-06-11T10:00:00Z", title="NoComp")])
            shipped = self.t.select_shipped(ctx.cfg, rows, "2026-06-10")
            out = self.t.render_changelog(ctx.cfg, shipped, "2026-06-10")
            self.assertIn("- #001 NoComp\n", out)
            self.assertNotIn("NoComp (", out)

    def test_child_nests_under_shipped_parent(self):
        with TemporaryDirectory() as tmp:
            ctx, rows = self.load(tmp, [
                row(1, closed="2026-06-11T10:00:00Z", kind="epic", title="Parent"),
                row(2, closed="2026-06-12T10:00:00Z", parent=1, title="Child"),
            ])
            shipped = self.t.select_shipped(ctx.cfg, rows, "2026-06-10")
            out = self.t.render_changelog(ctx.cfg, shipped, "2026-06-10")
            self.assertIn("- #001 Parent\n  - #002 Child\n", out)

    def test_orphan_child_renders_at_top_level(self):
        with TemporaryDirectory() as tmp:
            # parent #1 closed BEFORE since -> not in S; child #2 in S
            ctx, rows = self.load(tmp, [
                row(1, closed="2026-06-01T10:00:00Z", kind="epic", title="OldParent"),
                row(2, closed="2026-06-12T10:00:00Z", parent=1, title="Child"),
            ])
            shipped = self.t.select_shipped(ctx.cfg, rows, "2026-06-10")
            out = self.t.render_changelog(ctx.cfg, shipped, "2026-06-10")
            self.assertEqual(out.count("OldParent"), 0)       # parent not shown
            self.assertIn("- #002 Child\n", out)              # child at top level (no indent)

    def test_grandchildren_nest_two_levels(self):
        with TemporaryDirectory() as tmp:
            ctx, rows = self.load(tmp, [
                row(1, closed="2026-06-11T10:00:00Z", title="A"),
                row(2, closed="2026-06-11T10:00:00Z", parent=1, title="B"),
                row(3, closed="2026-06-11T10:00:00Z", parent=2, title="C"),
            ])
            shipped = self.t.select_shipped(ctx.cfg, rows, "2026-06-10")
            out = self.t.render_changelog(ctx.cfg, shipped, "2026-06-10")
            self.assertIn("- #001 A\n  - #002 B\n    - #003 C\n", out)

    def test_empty_renders_none(self):
        out = self.t.render_changelog({}, [], "2026-06-10")
        self.assertEqual(out, "## Shipped since 2026-06-10 — 0 issues\n\n_none_")
```

- [ ] **Step 2: Run to verify it fails**

Run: `python3 -m unittest tests.test_changelog.TestRenderChangelog -v`
Expected: FAIL — `AttributeError: module 'trck_engine' has no attribute 'render_changelog'`.

- [ ] **Step 3: Implement `render_changelog`**

In `trck`, add just below `select_shipped`:

```python
def render_changelog(cfg: dict, shipped: list, since: str) -> str:
    """Render the shipped set as nested markdown. Issues nest under their
    in-set parent; an issue whose parent is outside the set is a root. Siblings
    (and roots) are ordered by `closed` descending, id ascending on ties. The
    header counts the whole set regardless of nesting depth. Returns the markdown
    string (no trailing newline)."""
    n = len(shipped)
    header = f"## Shipped since {since} — {n} issue{'s' if n != 1 else ''}"
    if not shipped:
        return f"{header}\n\n_none_"

    g = Graph(cfg, shipped)
    out = [header, ""]

    def sib_sorted(items: list) -> list:
        xs = sorted(items, key=lambda r: r.id)            # id ascending
        xs.sort(key=lambda r: (r.closed or ""), reverse=True)  # closed desc, stable
        return xs

    def walk(node, depth: int, seen: set) -> None:
        comp = node.extra.get("component")
        tag = f" ({comp})" if comp else ""
        out.append("  " * depth + f"- #{node.id:03d} {node.title}{tag}")
        if node.id in seen:
            return
        for child in sib_sorted(g.children_of(node)):
            walk(child, depth + 1, seen | {node.id})

    roots = [r for r in shipped if r.parent is None or r.parent not in g.by_id]
    for root in sib_sorted(roots):
        walk(root, 0, set())
    return "\n".join(out)
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `python3 -m unittest tests.test_changelog.TestRenderChangelog -v`
Expected: PASS (6 tests).

- [ ] **Step 5: Commit**

```bash
git add trck tests/test_changelog.py
git commit -m "changelog: render_changelog nested-markdown output"
```

---

### Task 4: `cmd_changelog` + subparser wiring

**Files:**
- Modify: `trck` (add `cmd_changelog`; register the `changelog` subparser)
- Test: `tests/test_changelog.py`

- [ ] **Step 1: Write the failing test**

Append to `tests/test_changelog.py`:

```python
class TestCmdChangelog(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def load(self, tmp, rows_json):
        d = make_tracker(tmp, {})
        (d / "index.jsonl").write_text("".join(r + "\n" for r in rows_json))
        return d

    def run_cmd(self, d, since):
        buf = StringIO()
        with redirect_stdout(buf):
            self.t.cmd_changelog(ns(dir=str(d), since=since))
        return buf.getvalue()

    def test_end_to_end(self):
        with TemporaryDirectory() as tmp:
            d = self.load(tmp, [
                row(1, closed="2026-06-11T10:00:00Z", kind="epic", title="Parent", component="cli"),
                row(2, closed="2026-06-12T10:00:00Z", parent=1, title="Child", component="cli"),
                row(3, status="ongoing", title="Open"),                       # excluded
                row(4, closed="2026-06-11T10:00:00Z", resolution="wontfix"),   # excluded
            ])
            out = self.run_cmd(d, "2026-06-10")
            self.assertTrue(out.startswith("## Shipped since 2026-06-10 — 2 issues\n"))
            self.assertIn("- #001 Parent (cli)\n  - #002 Child (cli)\n", out)
            self.assertNotIn("Open", out)
            self.assertNotIn("#004", out)

    def test_empty_window(self):
        with TemporaryDirectory() as tmp:
            d = self.load(tmp, [row(1, closed="2026-06-01T10:00:00Z")])
            out = self.run_cmd(d, "2026-06-10")
            self.assertIn("— 0 issues", out)
            self.assertIn("_none_", out)

    def test_malformed_since_dies(self):
        with TemporaryDirectory() as tmp:
            d = self.load(tmp, [row(1, closed="2026-06-11T10:00:00Z")])
            with self.assertRaises(SystemExit):
                self.run_cmd(d, "last-tuesday")

    def test_changelog_is_a_registered_subcommand(self):
        p = self.t.build_parser()
        args = p.parse_args(["changelog", "--since", "2026-06-10"])
        self.assertIs(args.func, self.t.cmd_changelog)
        self.assertEqual(args.since, "2026-06-10")
```

- [ ] **Step 2: Run to verify it fails**

Run: `python3 -m unittest tests.test_changelog.TestCmdChangelog -v`
Expected: FAIL — `AttributeError: module 'trck_engine' has no attribute 'cmd_changelog'` (and the parser test fails because `changelog` isn't a subcommand yet).

- [ ] **Step 3: Implement `cmd_changelog` and register the subparser**

In `trck`, add `cmd_changelog` just below `render_changelog`:

```python
def cmd_changelog(args) -> None:
    ctx = build_ctx_or_die(args)
    since = parse_since(args.since)
    shipped = select_shipped(ctx.cfg, load_index(ctx), since)
    print(render_changelog(ctx.cfg, shipped, since))
```

Then register the subparser in `build_parser`, immediately before the
`ck = sub.add_parser("check", …)` block:

```python
    cl = sub.add_parser("changelog",
                        help="list issues shipped since a date/timestamp (release notes)",
                        description="Print, as nested markdown, the issues completed "
                                    "since --since: closed in a terminal status, "
                                    "excluding wontfix/duplicate/superseded. Children "
                                    "nest under their shipped parent.")
    cl.add_argument("--since", required=True, metavar="DATE|TIMESTAMP",
                    help="cutoff: a date (2026-06-10) or timestamp (2026-06-10T14:00:00Z)")
    cl.set_defaults(func=cmd_changelog)
```

- [ ] **Step 4: Run the test to verify it passes**

Run: `python3 -m unittest tests.test_changelog.TestCmdChangelog -v`
Expected: PASS (4 tests).

- [ ] **Step 5: Commit**

```bash
git add trck tests/test_changelog.py
git commit -m "changelog: cmd_changelog handler and subparser"
```

---

### Task 5: Full-suite regression + real-repo smoke

**Files:** none (verification only)

- [ ] **Step 1: Run the entire suite**

Run: `python3 -m unittest discover -s tests`
Expected: PASS — all modules green, including the new `tests.test_changelog`.

- [ ] **Step 2: Smoke the verb against this repo's tracker**

Run: `./trck changelog --since 2026-06-10`
Expected: a `## Shipped since 2026-06-10 — N issues` header followed by a nested markdown list of recently-closed issues (this repo has timestamped closes after the backfill). Sanity-check the nesting and that no `wontfix`/`duplicate`/`superseded` or open issues appear.

Run: `./trck changelog --since not-a-date`
Expected: a clean error message about the `--since` format and a nonzero exit (no traceback).

- [ ] **Step 3: Confirm help registration**

Run: `./trck changelog --help`
Expected: usage shows `--since` as required; the description mentions terminal status and the wontfix/duplicate/superseded exclusion.

- [ ] **Step 4: Commit (only if Step 1-3 required a fix)**

If a fix was needed, commit it; otherwise skip.

```bash
git add -A
git commit -m "changelog: fix surfaced by full-suite/smoke verification"
```

---

## Self-Review Notes

- **Spec coverage:** verb + subparser + `cmd_changelog` (Task 4) ✓; `--since` required + validated, malformed exits cleanly (Task 1 + Task 4) ✓; selection = terminal + `closed>=since` + no resolution, all kinds (Task 2) ✓; nested markdown, children under shipped parents, orphans at top level, two-space indent, `- #NNN Title (component)` (Task 3) ✓; siblings closed-desc, id tiebreak (Task 3 `sib_sorted`) ✓; header `## Shipped since <since> — N issues`, empty → `— 0 issues` + `_none_` (Task 3) ✓; no `--json` ✓; tests for selection/boundary/nesting/orphan/sort/header/empty/malformed (Tasks 1–4) ✓.
- **Placeholder scan:** none — every code/test step is complete.
- **Name consistency:** `SINCE_RE`, `parse_since`, `select_shipped(cfg, rows, since)`, `render_changelog(cfg, shipped, since)`, `cmd_changelog` used identically across tasks. Component accessed as `node.extra.get("component")` (custom field), never `node.component`.
- **Boundary correctness:** `closed >= since` with zero-padded ISO strings gives the documented bare-date and exact-timestamp inclusivity (Task 2 tests pin both).
