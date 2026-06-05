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
            k1 = self.base(id=2, slug="k1", parent=1, milestone="M0", status="done")
            k2 = self.base(id=3, slug="k2", parent=1, milestone="M1", status="ongoing")
            for r in (epic, k1, k2):
                self.write(ctx, r)
            self.t.save_index(ctx, [epic, k1, k2])
            text = self.t.generate_summary(ctx)
            self.assertIn("50% (1/2)", text)

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
            self.assertIn("50% (1/2)", text)  # rollup for a non-epic parent
            # the parent appears once (its Hierarchies heading), not also as a standalone item
            self.assertEqual(text.count("[#001"), 1)

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
