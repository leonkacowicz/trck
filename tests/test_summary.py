import unittest
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker


class TestSummary(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def ctx(self, tmp, config=None):
        d = make_tracker(tmp, config or {})
        return self.t.Ctx(d, self.t.load_config(d))

    def write(self, ctx, row):
        p = self.t.issue_path(ctx, row)
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text("# x\n")

    def base(self, **over):
        r = {"id": 1, "slug": "a", "title": "A", "kind": "task",
             "status": "backlog", "priority": "high", "depends_on": []}
        r.update(over)
        return r

    def test_counts_and_sections_in_config_order(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            rows = [self.base(id=1, slug="a", status="backlog"),
                    self.base(id=2, slug="b", status="ongoing"),
                    self.base(id=3, slug="c", status="done")]
            for r in rows:
                self.write(ctx, r)
            self.t.save_index(ctx, rows)
            text = self.t.generate_summary(ctx)
            self.assertIn("| backlog | 1 |", text)
            self.assertIn("| ongoing | 1 |", text)
            self.assertIn("| done | 1 |", text)
            self.assertLess(text.index("Backlog"), text.index("Ongoing"))

    def test_epic_rollup_counts_terminal(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            epic = self.base(id=1, slug="e", kind="epic", status="ongoing")
            k1 = self.base(id=2, slug="k1", parent=1, status="done")
            k2 = self.base(id=3, slug="k2", parent=1, status="ongoing")
            for r in (epic, k1, k2):
                self.write(ctx, r)
            self.t.save_index(ctx, [epic, k1, k2])
            text = self.t.generate_summary(ctx)
            self.assertIn("50% (1/2 pts · 1/2 done)", text)

    def test_nonepic_parent_gets_rollup(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            parent = self.base(id=1, slug="p", kind="task", status="ongoing")
            k1 = self.base(id=2, slug="k1", parent=1, status="done")
            k2 = self.base(id=3, slug="k2", parent=1, status="ongoing")
            for r in (parent, k1, k2):
                self.write(ctx, r)
            self.t.save_index(ctx, [parent, k1, k2])
            text = self.t.generate_summary(ctx)
            self.assertIn("50% (1/2 pts · 1/2 done)", text)  # rollup for a non-epic parent
            # the parent appears once (its Hierarchies heading), not also as a standalone item
            self.assertEqual(text.count("[#001"), 1)

    def test_status_section_sorted_by_priority_then_id(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)  # priorities: high, medium, low
            rows = [
                self.base(id=1, slug="a", status="backlog", priority="low"),
                self.base(id=2, slug="b", status="backlog", priority="high"),
                self.base(id=3, slug="c", status="backlog", priority="medium"),
                self.base(id=4, slug="d", status="backlog", priority="high"),
            ]
            for r in rows:
                self.write(ctx, r)
            self.t.save_index(ctx, rows)
            text = self.t.generate_summary(ctx)
            order = [text.index(f"[#{i:03d}") for i in (2, 4, 3, 1)]
            self.assertEqual(order, sorted(order),
                             "expected high (id-asc), then medium, then low")

    def test_status_section_unknown_priority_sorts_last(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            rows = [
                self.base(id=1, slug="a", status="backlog", priority="weird"),
                self.base(id=2, slug="b", status="backlog", priority="low"),
            ]
            for r in rows:
                self.write(ctx, r)
            self.t.save_index(ctx, rows)
            text = self.t.generate_summary(ctx)
            self.assertLess(text.index("[#002"), text.index("[#001"))

    def test_rollup_weighted_by_points(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            epic = self.base(id=1, slug="e", kind="epic", status="ongoing")
            k1 = self.base(id=2, slug="k1", parent=1, status="done", points=1)
            k2 = self.base(id=3, slug="k2", parent=1, status="ongoing", points=3)
            for r in (epic, k1, k2):
                self.write(ctx, r)
            self.t.save_index(ctx, [epic, k1, k2])
            text = self.t.generate_summary(ctx)
            # 1 of 4 points done -> 25%, though 1 of 2 issues is done
            self.assertIn("25% (1/4 pts · 1/2 done)", text)

    def test_rollup_recurses_to_leaf_descendants(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            epic = self.base(id=1, slug="e", kind="epic", status="ongoing")
            sub = self.base(id=2, slug="s", kind="epic", parent=1, status="ongoing")
            g1 = self.base(id=3, slug="g1", parent=2, status="done", points=2)
            g2 = self.base(id=4, slug="g2", parent=2, status="ongoing", points=2)
            leaf = self.base(id=5, slug="l", parent=1, status="done", points=1)
            for r in (epic, sub, g1, g2, leaf):
                self.write(ctx, r)
            self.t.save_index(ctx, [epic, sub, g1, g2, leaf])
            text = self.t.generate_summary(ctx)
            epic_line = [ln for ln in text.splitlines() if ln.startswith("### [#001")][0]
            # epic's leaves: g1(2,done) g2(2) leaf(1,done) -> 3/5 pts, 2/3 done
            self.assertIn("60% (3/5 pts · 2/3 done)", epic_line)

    def test_rollup_all_default_points_matches_count(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            epic = self.base(id=1, slug="e", kind="epic", status="ongoing")
            k1 = self.base(id=2, slug="k1", parent=1, status="done")
            k2 = self.base(id=3, slug="k2", parent=1, status="done")
            k3 = self.base(id=4, slug="k3", parent=1, status="ongoing")
            for r in (epic, k1, k2, k3):
                self.write(ctx, r)
            self.t.save_index(ctx, [epic, k1, k2, k3])
            text = self.t.generate_summary(ctx)
            # all weights default to 1 -> points pct equals count pct
            self.assertIn("67% (2/3 pts · 2/3 done)", text)

    def test_rollup_zero_total_points_guard(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            epic = self.base(id=1, slug="e", kind="epic", status="ongoing")
            k1 = self.base(id=2, slug="k1", parent=1, status="done", points=0)
            for r in (epic, k1):
                self.write(ctx, r)
            self.t.save_index(ctx, [epic, k1])
            text = self.t.generate_summary(ctx)
            self.assertIn("0% (0/0 pts · 1/1 done)", text)  # no ZeroDivisionError

    def test_rollup_survives_parent_cycle(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            a = self.base(id=1, slug="a", parent=2, status="ongoing")
            b = self.base(id=2, slug="b", parent=1, status="ongoing")
            self.write(ctx, a); self.write(ctx, b)
            self.t.save_index(ctx, [a, b])
            text = self.t.generate_summary(ctx)  # must terminate, not hang
            self.assertIn("Hierarchies", text)

    def test_custom_statuses_render(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp, {"statuses": [
                {"name": "todo", "role": "initial"},
                {"name": "shipped", "role": "terminal"}]})
            r = self.base(id=1, slug="a", status="todo")
            self.write(ctx, r)
            self.t.save_index(ctx, [r])
            text = self.t.generate_summary(ctx)
            self.assertIn("| todo | 1 |", text)
            self.assertIn("Todo", text)
