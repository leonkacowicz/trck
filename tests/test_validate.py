import unittest
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker


class TestValidate(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def ctx(self, tmp, config=None):
        d = make_tracker(tmp, config or {})
        return self.t.Ctx(d, self.t.load_config(d))

    def write(self, ctx, row, body="# x\n"):
        p = self.t.issue_path(ctx, row)
        p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(body)

    def base(self, **over):
        fields = {"id": 1, "slug": "a", "title": "A", "kind": "task",
                  "status": "backlog", "priority": "high", "depends_on": []}
        fields.update(over)
        return self.t.Issue(**fields)

    def test_clean_tracker_has_no_errors(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = self.base()
            self.write(ctx, row)
            self.t.save_index(ctx, [row])
            errors, _ = self.t.validate(ctx)
            self.assertEqual(errors, [])

    def test_status_folder_mismatch_is_error(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = self.base()
            self.write(ctx, row)               # file is in backlog/
            row2 = self.base(status="done")    # but index says done
            self.t.save_index(ctx, [row2])
            errors, _ = self.t.validate(ctx)
            self.assertTrue(any("status" in e for e in errors))

    def test_parent_can_be_any_kind(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            p = self.base(id=1, slug="p", kind="task")  # a task, not an epic
            c = self.base(id=2, slug="c", parent=1)
            self.write(ctx, p); self.write(ctx, c)
            self.t.save_index(ctx, [p, c])
            errors, _ = self.t.validate(ctx)
            self.assertEqual(errors, [])  # a non-epic parent is allowed

    def test_parent_must_exist(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            c = self.base(id=2, slug="c", parent=99)  # no such parent
            self.write(ctx, c)
            self.t.save_index(ctx, [c])
            errors, _ = self.t.validate(ctx)
            self.assertTrue(any("does not exist" in e for e in errors))

    def test_negative_leaf_points_is_error(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = self.base(points=-1)
            self.write(ctx, row)
            self.t.save_index(ctx, [row])
            errors, _ = self.t.validate(ctx)
            self.assertTrue(any("points" in e for e in errors))

    def test_non_integer_points_is_error(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = self.base(points="lots")
            self.write(ctx, row)
            self.t.save_index(ctx, [row])
            errors, _ = self.t.validate(ctx)
            self.assertTrue(any("points" in e for e in errors))

    def test_parent_carrying_own_points_is_error(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            p = self.base(id=1, slug="p", points=5)       # has children but a stored weight
            c = self.base(id=2, slug="c", parent=1)
            self.write(ctx, p); self.write(ctx, c)
            self.t.save_index(ctx, [p, c])
            errors, _ = self.t.validate(ctx)
            self.assertTrue(any("points" in e for e in errors))

    def test_terminal_role_drives_warnings(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            epic = self.base(id=1, slug="e", kind="epic", status="ongoing")
            child = self.base(id=2, slug="c", parent=1, status="done")
            self.write(ctx, epic); self.write(ctx, child)
            self.t.save_index(ctx, [epic, child])
            _, warnings = self.t.validate(ctx)
            self.assertTrue(any("all children" in w for w in warnings))
