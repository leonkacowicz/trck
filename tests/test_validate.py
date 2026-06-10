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

    def test_non_integer_points_fails_loud_at_load(self):
        # wrong *type* (not a wrong value): structurally invalid, so load dies
        # rather than deferring a soft error to validate.
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = self.base(points="lots")
            self.write(ctx, row)
            self.t.save_index(ctx, [row])
            with self.assertRaises(SystemExit):
                self.t.validate(ctx)  # reloads the index -> from_dict rejects it

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

    def test_two_node_dependency_cycle_is_error(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            a = self.base(id=1, slug="a", depends_on=[2])
            b = self.base(id=2, slug="b", depends_on=[1])
            self.write(ctx, a); self.write(ctx, b)
            self.t.save_index(ctx, [a, b])
            errors, _ = self.t.validate(ctx)
            self.assertTrue(any("dependency cycle" in e for e in errors))

    def test_longer_dependency_cycle_reported_once(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            a = self.base(id=1, slug="a", depends_on=[2])
            b = self.base(id=2, slug="b", depends_on=[3])
            c = self.base(id=3, slug="c", depends_on=[1])
            self.write(ctx, a); self.write(ctx, b); self.write(ctx, c)
            self.t.save_index(ctx, [a, b, c])
            errors, _ = self.t.validate(ctx)
            cyc = [e for e in errors if "dependency cycle" in e]
            self.assertEqual(len(cyc), 1)  # one error per cycle, not one per node

    def test_self_dependency_is_error(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            a = self.base(id=1, slug="a", depends_on=[1])
            self.write(ctx, a)
            self.t.save_index(ctx, [a])
            errors, _ = self.t.validate(ctx)
            self.assertTrue(any("dependency cycle" in e for e in errors))

    def test_valid_dep_dag_has_no_cycle_errors(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            # diamond: d depends on b and c; b and c both depend on a. No cycle.
            a = self.base(id=1, slug="a")
            b = self.base(id=2, slug="b", depends_on=[1])
            c = self.base(id=3, slug="c", depends_on=[1])
            d = self.base(id=4, slug="d", depends_on=[2, 3])
            for r in (a, b, c, d):
                self.write(ctx, r)
            self.t.save_index(ctx, [a, b, c, d])
            errors, _ = self.t.validate(ctx)
            self.assertFalse(any("dependency cycle" in e for e in errors))

    def test_self_parent_reported_as_parent_cycle(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            a = self.base(id=1, slug="a", parent=1)  # points at itself
            self.write(ctx, a)
            self.t.save_index(ctx, [a])
            errors, _ = self.t.validate(ctx)
            self.assertIn("#001 is in a parent cycle", errors)

    def test_two_node_parent_cycle_flags_both(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            a = self.base(id=1, slug="a", parent=2)
            b = self.base(id=2, slug="b", parent=1)
            self.write(ctx, a); self.write(ctx, b)
            self.t.save_index(ctx, [a, b])
            errors, _ = self.t.validate(ctx)
            cyc = [e for e in errors if "parent cycle" in e]
            self.assertEqual(sorted(cyc),
                             ["#001 is in a parent cycle", "#002 is in a parent cycle"])

    def test_clean_parent_spine_has_no_cycle_error(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            p = self.base(id=1, slug="p")
            c = self.base(id=2, slug="c", parent=1)
            self.write(ctx, p); self.write(ctx, c)
            self.t.save_index(ctx, [p, c])
            errors, _ = self.t.validate(ctx)
            self.assertFalse(any("parent cycle" in e for e in errors))

    def test_preloaded_rows_skip_the_index_reread(self):
        # validate() accepts already-loaded rows and validates those against the
        # on-disk file scan, without re-parsing index.jsonl.
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            row = self.base()
            self.write(ctx, row)
            self.t.save_index(ctx, [row])
            calls = []
            orig = self.t.load_index
            self.t.load_index = lambda c: (calls.append(1), orig(c))[1]
            try:
                errors, warnings = self.t.validate(ctx, [row])
            finally:
                self.t.load_index = orig
            self.assertEqual(errors, [])
            self.assertEqual(calls, [])  # rows supplied -> no index re-read
            # identical to the reloading path
            self.assertEqual((errors, warnings), self.t.validate(ctx))

    def test_omitting_rows_still_reloads_from_disk(self):
        # The default path must still read (and re-parse) the persisted index so
        # finalize's "validate the persisted state" intent holds for callers that
        # don't pass rows.
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            calls = []
            orig = self.t.load_index
            self.t.load_index = lambda c: (calls.append(1), orig(c))[1]
            try:
                self.t.validate(ctx)
            finally:
                self.t.load_index = orig
            self.assertEqual(calls, [1])
