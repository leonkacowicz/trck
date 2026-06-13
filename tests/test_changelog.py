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
            with self.subTest(bad=bad), self.assertRaises(SystemExit):
                self.t.parse_since(bad)


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

    def test_children_sorted_closed_descending_under_parent(self):
        with TemporaryDirectory() as tmp:
            ctx, rows = self.load(tmp, [
                row(1, closed="2026-06-10T10:00:00Z", title="P"),
                row(2, closed="2026-06-11T10:00:00Z", parent=1, title="Older"),
                row(3, closed="2026-06-13T10:00:00Z", parent=1, title="Newer"),
            ])
            shipped = self.t.select_shipped(ctx.cfg, rows, "2026-06-01")
            out = self.t.render_changelog(ctx.cfg, shipped, "2026-06-01")
            # both children nest under #1, newest-closed child first
            self.assertIn("- #001 P\n  - #003 Newer\n  - #002 Older\n", out)

    def test_empty_renders_none(self):
        out = self.t.render_changelog({}, [], "2026-06-10")
        self.assertEqual(out, "## Shipped since 2026-06-10 — 0 issues\n\n_none_\n")


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


if __name__ == "__main__":
    unittest.main()
