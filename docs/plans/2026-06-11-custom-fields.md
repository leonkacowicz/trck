# Free-form Custom Fields Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let any issue carry arbitrary free-form `key=value` metadata (e.g. `assignee`, `component`) that can be set, filtered, and sorted — without touching `trck.json` or the issue body.

**Architecture:** No storage change. The `Issue.extra` dict already round-trips unknown keys through `index.jsonl`. This plan adds the user-facing surface: a shared key-validation helper, write support on `set` (`--field`/`--unset`), an integrity rule in `validate`, and read support on `list` (`--field` filter, `--sort field:NAME`, `--show-field` column). Values are always strings; sorting is lexicographic.

**Tech Stack:** Python 3.12+ standard library only. Tests: `unittest`, run via `python3 -m unittest discover -s tests -v`. The engine is the single extensionless file `./trck`; tests import it through `tests/helpers.py::load_trck()`.

**Spec:** `docs/specs/2026-06-11-custom-fields-design.md`

**Tracker:** epic #048; tasks #049–#054.

---

## File Structure

- **`trck`** (modify) — the engine. Touched in five places:
  - new module-level regex `FIELD_KEY_RE` (next to `SLUG_RE`, ~line 28)
  - new helper `check_field_key` (after `FIELD_DEFAULTS`, ~line 236)
  - `cmd_set` (~line 1300) + its argparse block (~line 1898)
  - `validate` per-issue loop (~line 674)
  - `cmd_list` (~line 1408), `print_rows` (~line 1180-ish), + the `list` argparse block (~line 1959)
  - help/epilog/CLAUDE template text (~lines 1163, 1836) and `issues/.../README` snippet
- **`tests/test_custom_fields.py`** (create) — all set/list/validate behavior for custom fields.

Each task below is independently committable and ordered to match the tracker (#049 first; the rest build on its shared helper).

---

## Task 1 (#049): `set --field`/`--unset` with key guards

**Files:**
- Modify: `trck` — add `FIELD_KEY_RE` (~line 28), `check_field_key` (~line 236), the mutation block in `cmd_set` (~line 1299), and the `set` argparse flags (~line 1910).
- Test: `tests/test_custom_fields.py`

- [ ] **Step 1: Write the failing test**

Create `tests/test_custom_fields.py`:

```python
import io
import json
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestCustomFields(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    # -- helpers -------------------------------------------------------------
    def seed(self, d, **over):
        args = ns(dir=str(d), title=over.pop("title", "Item"),
                  priority=over.pop("priority", "high"), kind=over.pop("kind", None),
                  parent=None, depends=None, spec=None, slug=None, points=None)
        self.t.cmd_new(args)

    def set_(self, d, iid, **over):
        args = ns(dir=str(d), id=iid, priority=None, points=None, parent=None,
                  spec=None, kind=None, title=None, slug=None,
                  field=over.pop("field", None), unset=over.pop("unset", None))
        self.t.cmd_set(args)

    def rows(self, d):
        ctx = self.t.Ctx(d, self.t.load_config(d))
        return {r.id: r for r in self.t.load_index(ctx)}

    def raw(self, d):
        return (Path(d) / "index.jsonl").read_text()

    # -- write side ----------------------------------------------------------
    def test_field_sets_value(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.set_(d, 1, field=["assignee=leon"])
            self.assertEqual(self.rows(d)[1].extra, {"assignee": "leon"})
```

- [ ] **Step 2: Run it to verify it fails**

Run: `python3 -m unittest tests.test_custom_fields.TestCustomFields.test_field_sets_value -v`
Expected: FAIL — `cmd_set` does not yet read `args.field`, so `extra` stays `{}` (AssertionError), or AttributeError if `field` is unhandled.

- [ ] **Step 3: Add the regex and helper**

In `trck`, immediately after `SLUG_RE`/`FILENAME_RE` (~line 29), add:

```python
FIELD_KEY_RE = re.compile(r"^[a-z][a-z0-9_-]*$")
```

After the `FIELD_DEFAULTS = { ... }` block (~line 236), add:

```python
def check_field_key(key: str) -> str | None:
    """Validate a custom-field key. Returns an error message, or None if OK.
    Custom fields are free-form, but their keys must be slug-like and must not
    collide with a built-in field name (use the matching flag/verb for those)."""
    if key in CANON_KEYS:
        return f"'{key}' is a built-in field; use its flag/verb, not --field/--unset"
    if not FIELD_KEY_RE.match(key):
        return f"invalid field key '{key}' (must match [a-z][a-z0-9_-]*)"
    return None
```

- [ ] **Step 4: Add the mutation block to `cmd_set`**

In `cmd_set`, after the `if args.kind:` block and before `old = issue_path(ctx, row)` (~line 1300), insert:

```python
    for spec in (getattr(args, "field", None) or []):
        if "=" not in spec:
            die(f"--field expects key=value, got '{spec}'")
        key, val = spec.split("=", 1)
        if (m := check_field_key(key)):
            die(m)
        if val == "":
            row.extra.pop(key, None)  # empty value clears (alias for --unset)
        else:
            row.extra[key] = val
    for key in (getattr(args, "unset", None) or []):
        if (m := check_field_key(key)):
            die(m)
        row.extra.pop(key, None)
```

- [ ] **Step 5: Add the argparse flags**

In the `set` subparser (`se = sub.add_parser("set", ...)`), after the `--slug` line (~line 1910), add:

```python
    se.add_argument("--field", action="append", metavar="KEY=VALUE",
                    help="set a custom field (repeatable); empty value clears it")
    se.add_argument("--unset", action="append", metavar="KEY",
                    help="remove a custom field (repeatable)")
```

- [ ] **Step 6: Run the test to verify it passes**

Run: `python3 -m unittest tests.test_custom_fields.TestCustomFields.test_field_sets_value -v`
Expected: PASS

- [ ] **Step 7: Add the remaining write-side tests**

Append to `TestCustomFields`:

```python
    def test_field_overwrites(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.set_(d, 1, field=["assignee=leon"])
            self.set_(d, 1, field=["assignee=mara"])
            self.assertEqual(self.rows(d)[1].extra, {"assignee": "mara"})

    def test_multiple_fields_one_call(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.set_(d, 1, field=["assignee=leon", "component=ui"])
            self.assertEqual(self.rows(d)[1].extra,
                             {"assignee": "leon", "component": "ui"})

    def test_unset_removes(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.set_(d, 1, field=["assignee=leon"])
            self.set_(d, 1, unset=["assignee"])
            self.assertEqual(self.rows(d)[1].extra, {})

    def test_empty_value_clears(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.set_(d, 1, field=["assignee=leon"])
            self.set_(d, 1, field=["assignee="])
            self.assertEqual(self.rows(d)[1].extra, {})

    def test_reserved_key_rejected(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            with self.assertRaises(SystemExit):
                self.set_(d, 1, field=["status=foo"])

    def test_malformed_key_rejected(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            for bad in ["Assignee=x", "1tag=x", "a b=x"]:
                with self.assertRaises(SystemExit):
                    self.set_(d, 1, field=[bad])

    def test_field_persists_in_index(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.set_(d, 1, field=["component=ui", "assignee=leon"])
            line = json.loads(self.raw(d).splitlines()[0])
            # extras written after known fields, in sorted key order
            self.assertEqual(line["assignee"], "leon")
            self.assertEqual(line["component"], "ui")
            keys = list(line)
            self.assertLess(keys.index("assignee"), keys.index("component"))
```

- [ ] **Step 8: Run the whole class**

Run: `python3 -m unittest tests.test_custom_fields -v`
Expected: all PASS. (`die` raises `SystemExit`, satisfying the rejection tests.)

- [ ] **Step 9: Commit**

```bash
git add trck tests/test_custom_fields.py
git commit -m "set: --field/--unset for free-form custom fields"
```

---

## Task 2 (#050): `validate`/`check` integrity rule

**Files:**
- Modify: `trck` — `validate` per-issue loop (~line 674).
- Test: `tests/test_custom_fields.py`

- [ ] **Step 1: Write the failing test**

Append to `TestCustomFields`:

```python
    def test_check_passes_with_custom_fields(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            self.set_(d, 1, field=["assignee=leon"])
            ctx = self.t.Ctx(d, self.t.load_config(d))
            errors, _ = self.t.validate(ctx)
            self.assertEqual(errors, [])

    def test_validate_flags_non_string_extra(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            # hand-corrupt the index: a non-string custom value
            p = Path(d) / "index.jsonl"
            row = json.loads(p.read_text().splitlines()[0])
            row["estimate"] = 5  # int, not a string
            p.write_text(json.dumps(row) + "\n")
            ctx = self.t.Ctx(d, self.t.load_config(d))
            errors, _ = self.t.validate(ctx)
            self.assertTrue(any("estimate" in e for e in errors), errors)
```

- [ ] **Step 2: Run to verify the second test fails**

Run: `python3 -m unittest tests.test_custom_fields.TestCustomFields.test_validate_flags_non_string_extra -v`
Expected: FAIL — `validate` has no extra-field rule yet, so `errors` is empty.

- [ ] **Step 3: Add the rule to `validate`**

In `validate`, inside the `for iid, r in by_id.items():` loop, after the resolution check (the `if r.resolution is not None ...` block, ~line 674), add:

```python
        for k, v in r.extra.items():
            if not FIELD_KEY_RE.match(k):
                errors.append(f"#{iid:03d} bad custom field key '{k}'")
            elif not isinstance(v, str):
                errors.append(f"#{iid:03d} custom field '{k}' must be a string, got {v!r}")
```

- [ ] **Step 4: Run both validate tests**

Run: `python3 -m unittest tests.test_custom_fields.TestCustomFields.test_check_passes_with_custom_fields tests.test_custom_fields.TestCustomFields.test_validate_flags_non_string_extra -v`
Expected: both PASS

- [ ] **Step 5: Commit**

```bash
git add trck tests/test_custom_fields.py
git commit -m "validate: custom field keys well-formed, values are strings"
```

---

## Task 3 (#051): `list --field` exact-match filter

**Files:**
- Modify: `trck` — `cmd_list` (~line 1408) + the `list` argparse block (~line 1959).
- Test: `tests/test_custom_fields.py`

- [ ] **Step 1: Write the failing test**

Append a `list` helper and tests to `TestCustomFields`:

```python
    def list_(self, d, **over):
        args = ns(dir=str(d), id=None, flat=True, status=None, kind=None,
                  priority=None, label=None, parent=None, match=None,
                  sort=None, blocked=False, orphan=False, paths=False,
                  field=over.pop("field", None),
                  show_field=over.pop("show_field", None))
        for k, v in over.items():
            setattr(args, k, v)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_list(args)
        return buf.getvalue()

    def test_field_filter(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, title="Alpha")
            self.seed(d, title="Beta")
            self.set_(d, 1, field=["assignee=leon"])
            self.set_(d, 2, field=["assignee=mara"])
            out = self.list_(d, field=["assignee=leon"])
            self.assertIn("Alpha", out)
            self.assertNotIn("Beta", out)

    def test_field_filter_anded(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, title="Alpha")
            self.seed(d, title="Beta")
            self.set_(d, 1, field=["assignee=leon", "component=ui"])
            self.set_(d, 2, field=["assignee=leon", "component=api"])
            out = self.list_(d, field=["assignee=leon", "component=ui"])
            self.assertIn("Alpha", out)
            self.assertNotIn("Beta", out)
```

- [ ] **Step 2: Run to verify failure**

Run: `python3 -m unittest tests.test_custom_fields.TestCustomFields.test_field_filter -v`
Expected: FAIL — `cmd_list` ignores `args.field`, so Beta still appears.

- [ ] **Step 3: Parse the filters in `cmd_list`**

In `cmd_list`, after `only_orphan = getattr(args, "orphan", False)` (~line 1415) and before `def keep(r):`, add:

```python
    field_filters = {}
    for spec in (getattr(args, "field", None) or []):
        if "=" not in spec:
            die(f"--field expects key=value, got '{spec}'")
        k, v = spec.split("=", 1)
        field_filters[k] = v
```

- [ ] **Step 4: Add the predicate clause**

In the `keep(r)` return expression, add a final clause (before the closing `)`):

```python
                and all(r.extra.get(k) == v for k, v in field_filters.items())
```

- [ ] **Step 5: Add the argparse flag**

In the `list` subparser, after the `--match` line (~line 1978), add:

```python
    ls.add_argument("--field", action="append", metavar="KEY=VALUE",
                    help="filter to issues whose custom field KEY equals VALUE "
                         "(repeatable; multiple are AND-ed)")
```

- [ ] **Step 6: Run the filter tests**

Run: `python3 -m unittest tests.test_custom_fields.TestCustomFields.test_field_filter tests.test_custom_fields.TestCustomFields.test_field_filter_anded -v`
Expected: both PASS

- [ ] **Step 7: Commit**

```bash
git add trck tests/test_custom_fields.py
git commit -m "list: --field exact-match filter (AND-ed)"
```

---

## Task 4 (#052): `list --sort field:NAME`

**Files:**
- Modify: `trck` — `cmd_list` sort block (~line 1428) + the `--sort` argparse line (~line 1979).
- Test: `tests/test_custom_fields.py`

- [ ] **Step 1: Write the failing test**

Append to `TestCustomFields`:

```python
    def _order(self, out):
        # the leading "#NNN" of each printed row, in print order
        import re
        return re.findall(r"#(\d{3})", out)

    def test_sort_by_field_missing_last(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, title="One")    # #1 -> zebra
            self.seed(d, title="Two")    # #2 -> alpha
            self.seed(d, title="Three")  # #3 -> (unset)
            self.set_(d, 1, field=["owner=zebra"])
            self.set_(d, 2, field=["owner=alpha"])
            out = self.list_(d, sort="field:owner")
            self.assertEqual(self._order(out), ["002", "001", "003"])
```

- [ ] **Step 2: Run to verify failure**

Run: `python3 -m unittest tests.test_custom_fields.TestCustomFields.test_sort_by_field_missing_last -v`
Expected: FAIL — `--sort field:owner` is not handled (rows stay id-ordered, so order is `001, 002, 003`).

- [ ] **Step 3: Handle the `field:` sort in `cmd_list`**

Replace the existing sort block in `cmd_list`:

```python
    sort = getattr(args, "sort", None) or "id"
    sort_keys = {
        "priority": lambda r: (priority_rank(ctx.cfg, r.priority), r.id),
        "points": lambda r: (-r.points, r.id),
        "created": lambda r: (r.created or "", r.id),
        "id": get_id,
    }
    key = sort_keys.get(sort, get_id)
```

with:

```python
    sort = getattr(args, "sort", None) or "id"
    if sort.startswith("field:"):
        fname = sort[len("field:"):]
        if not fname:
            die("--sort field: needs a field name (e.g. --sort field:assignee)")
        # present rows (group 0) sort by value then id; missing rows (group 1) sort last
        key = lambda r: (0, r.extra[fname], r.id) if fname in r.extra else (1, "", r.id)
    else:
        sort_keys = {
            "priority": lambda r: (priority_rank(ctx.cfg, r.priority), r.id),
            "points": lambda r: (-r.points, r.id),
            "created": lambda r: (r.created or "", r.id),
            "id": get_id,
        }
        if sort not in sort_keys:
            die(f"unknown --sort '{sort}' "
                "(choices: id, priority, points, created, field:NAME)")
        key = sort_keys[sort]
```

- [ ] **Step 4: Loosen the argparse `--sort`**

`--sort` currently has `choices=[...]`, which would reject `field:NAME`. Replace the `--sort` line (~line 1979) with:

```python
    ls.add_argument("--sort", metavar="FIELD",
                    help="order by id (default), priority, points, created, or "
                         "field:NAME for a custom field (missing values sort last)")
```

- [ ] **Step 5: Run the sort test**

Run: `python3 -m unittest tests.test_custom_fields.TestCustomFields.test_sort_by_field_missing_last -v`
Expected: PASS (order `002, 001, 003`)

- [ ] **Step 6: Guard against regressions from dropping `choices`**

Run: `python3 -m unittest tests.test_read tests.test_presentation -v`
Expected: PASS. If a test asserted argparse rejects a bad `--sort` value, it now reaches `cmd_list` and hits `die(...)`, which still raises `SystemExit` — update that test's expected message only if it asserts on text.

- [ ] **Step 7: Commit**

```bash
git add trck tests/test_custom_fields.py
git commit -m "list: --sort field:NAME (missing values last)"
```

---

## Task 5 (#053): `list --show-field` column

**Files:**
- Modify: `trck` — `print_rows` (~line 1180) and `cmd_list` (the two `print_rows` calls + arg gather) + the `list` argparse block.
- Test: `tests/test_custom_fields.py`

- [ ] **Step 1: Write the failing test**

Append to `TestCustomFields`:

```python
    def test_show_field_column(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, title="Alpha")
            self.seed(d, title="Beta")
            self.set_(d, 1, field=["component=ui"])
            out = self.list_(d, show_field=["component"])
            # #1 shows the value, #2 (no value) shows no component tag
            line1 = next(l for l in out.splitlines() if "#001" in l)
            line2 = next(l for l in out.splitlines() if "#002" in l)
            self.assertIn("component=ui", line1)
            self.assertNotIn("component=", line2)
```

- [ ] **Step 2: Run to verify failure**

Run: `python3 -m unittest tests.test_custom_fields.TestCustomFields.test_show_field_column -v`
Expected: FAIL — no `component=ui` in the output (`print_rows` ignores custom fields).

- [ ] **Step 3: Extend `print_rows`**

Change the signature (~line 1180) from:

```python
def print_rows(ctx: Ctx, rows: list[Issue], annotate=None, prefix=None, dim=None) -> None:
```

to:

```python
def print_rows(ctx: Ctx, rows: list[Issue], annotate=None, prefix=None, dim=None,
               show_fields=None) -> None:
```

Inside the `for r in rows:` loop, after `ann = annotate(r) if annotate else ""`, add:

```python
        fsuf = ""
        if show_fields:
            segs = [f"{n}={r.extra[n]}" for n in show_fields if n in r.extra]
            if segs:
                fsuf = "  " + paint(" ".join(segs), "dim")
```

Append `+ fsuf` to **both** print statements in the loop:
- the dimmed-ancestor branch: `print(paint(body, "dim") + ann + fsuf)`
- the normal branch: change the trailing `...{tagstr}{ann}")` to `...{tagstr}{ann}{fsuf}")`

- [ ] **Step 4: Thread `show_fields` through `cmd_list`**

In `cmd_list`, near the other arg reads (after `field_filters` from Task 3), add:

```python
    show_fields = getattr(args, "show_field", None) or []
```

Pass `show_fields=show_fields` to every `print_rows(...)` call inside `cmd_list` (the `--flat` branch ~line 1445 and the nested-forest branch). Leave `print_rows` calls in `cmd_ready`/`cmd_next` untouched (they default to `None`).

- [ ] **Step 5: Add the argparse flag**

In the `list` subparser, after the `--field` flag from Task 3, add:

```python
    ls.add_argument("--show-field", action="append", metavar="NAME", dest="show_field",
                    help="append a custom field's value as a trailing column "
                         "(repeatable); list is otherwise unchanged")
```

- [ ] **Step 6: Run the show-field test**

Run: `python3 -m unittest tests.test_custom_fields.TestCustomFields.test_show_field_column -v`
Expected: PASS

- [ ] **Step 7: Commit**

```bash
git add trck tests/test_custom_fields.py
git commit -m "list: --show-field opt-in trailing column"
```

---

## Task 6 (#054): docs — help, epilog, README, spec link

**Files:**
- Modify: `trck` — CLAUDE template verbs (~line 1163), the `TYPICAL FLOW` epilog (~line 1838), README root if present.
- Test: `tests/test_help.py` (verify the new example renders).

- [ ] **Step 1: Write the failing test**

`tests/test_help.py` uses `self.parser = self.t.build_parser()` in `setUp`, a `norm()` whitespace-collapse helper, and a `sub_help(name)` helper that returns a subcommand's formatted help. Match that style. Append to `TestHelp`:

```python
    def test_set_help_documents_custom_fields(self):
        h = self.sub_help("set")
        self.assertIn("--field", h)
        self.assertIn("custom field", h)

    def test_top_level_epilog_shows_custom_field_example(self):
        h = norm(self.parser.format_help())
        self.assertIn("--field assignee=leon", h)
```

- [ ] **Step 2: Run to verify failure**

Run: `python3 -m unittest tests.test_help.TestHelp.test_set_help_documents_custom_fields tests.test_help.TestHelp.test_top_level_epilog_shows_custom_field_example -v`
Expected: `test_set_help_documents_custom_fields` already PASSES (Task 1 added the `set --field` flag with "custom field" help — this test is a regression guard); `test_top_level_epilog_shows_custom_field_example` FAILS — the epilog has no custom-field example until Step 3.

- [ ] **Step 3: Update the `TYPICAL FLOW` epilog**

In the `main` epilog (~line 1838), after the `trck set 7 --points 3 --parent 4` line, add:

```
  trck set 7 --field assignee=leon --field component=ui  # arbitrary metadata
```

and after the `trck list --match parser --orphan` line, add:

```
  trck list --field assignee=leon --sort field:component # filter + sort custom fields
```

- [ ] **Step 4: Update the CLAUDE template verb list**

In `CLAUDE_TEMPLATE` (~line 1163), change the `trck set` line to include the new flags:

```
- `trck set NNN [--priority …] [--parent …|none] [--kind …] [--title …] [--field k=v] [--unset k]`
```

and add, after the `trck label` line (~line 1165):

```
- Custom fields: `trck set NNN --field assignee=leon`; filter `trck list --field assignee=leon`; sort `--sort field:assignee`; show `--show-field assignee`.
```

- [ ] **Step 5: Update the root README**

In the repo-root `README.md`, find the feature/usage list and add one bullet describing custom fields, e.g.:

```markdown
- **Custom fields** — attach arbitrary `key=value` metadata to any issue
  (`trck set N --field assignee=leon`) and filter/sort on it
  (`trck list --field assignee=leon --sort field:assignee`). Free-form by design;
  see `docs/specs/2026-06-11-custom-fields-design.md`.
```

(If the README has no feature list, add a short "Custom fields" subsection near the existing command examples instead.)

- [ ] **Step 6: Run the help test + full suite**

Run: `python3 -m unittest tests.test_help -v`
Expected: PASS

Run: `python3 -m unittest discover -s tests -v`
Expected: all PASS

- [ ] **Step 7: Commit**

```bash
git add trck README.md tests/test_help.py
git commit -m "docs: custom fields in help, epilog, README"
```

---

## Final verification & tracker close-out

- [ ] **Run the full suite**

Run: `python3 -m unittest discover -s tests -v`
Expected: all PASS (including the existing 20+ test modules).

- [ ] **Self-host check**

Run: `./trck check`
Expected: `OK — N issues, 0 errors, 0 warning(s)`.

- [ ] **Smoke test the real surface on this repo's tracker** (read-only; do not leave stray fields)

```bash
./trck set 49 --field assignee=leon
./trck list --field assignee=leon --show-field assignee
./trck list --sort field:assignee
./trck set 49 --unset assignee     # clean up
./trck check
```

Expected: #049 appears with `assignee=leon`, then the field is gone and `check` is clean.

- [ ] **Close the tracker tasks** as each is completed:

```bash
./trck done 49   # repeat for 50..54 as their tasks land
./trck done 48   # epic, once all children are done
```

---

## Self-Review notes

- **Spec coverage:** set (`--field`/`--unset`, empty-clears) → Task 1; key-shape + reserved-key guards → Task 1 (`check_field_key`); validate string/keys rule → Task 2; `list --field` AND filter → Task 3; `--sort field:NAME` missing-last → Task 4; `--show-field` column → Task 5; `show` already prints extras (no code, covered by existing behavior); docs → Task 6. All spec sections map to a task.
- **Type consistency:** `check_field_key(key) -> str | None` is defined in Task 1 and reused in Task 2's validate rule; `FIELD_KEY_RE` defined once (Task 1) and reused (Task 2). `print_rows(..., show_fields=None)` (Task 5) matches the call sites updated in the same task. The namespace attrs the tests set (`field`, `unset`, `show_field`) match the `getattr(args, ...)` names and argparse `dest`s.
- **Out of scope (unchanged):** declared schemas, typed sort, presence/absence filters beyond empty-clear — per spec.
