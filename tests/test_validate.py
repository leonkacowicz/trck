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

    def test_missing_active_role_is_config_error(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp, {"statuses": [
                {"name": "backlog", "role": "initial"},
                {"name": "done", "role": "terminal"}]})
            row = self.base()
            self.write(ctx, row)
            self.t.save_index(ctx, [row])
            errors, _ = self.t.validate(ctx)
            self.assertTrue(any("active" in e and "role" in e for e in errors))

    def test_duplicate_initial_role_is_config_error(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp, {"statuses": [
                {"name": "backlog", "role": "initial"},
                {"name": "triage", "role": "initial"},
                {"name": "ongoing", "role": "active"},
                {"name": "done", "role": "terminal"}]})
            row = self.base()
            self.write(ctx, row)
            self.t.save_index(ctx, [row])
            errors, _ = self.t.validate(ctx)
            self.assertTrue(any("initial" in e and "role" in e for e in errors))

    def test_non_pinned_parent_off_its_rollup_is_error(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            p = self.base(id=1, slug="p", status="done")        # claims done...
            c = self.base(id=2, slug="c", parent=1, status="backlog")  # ...child still open
            self.write(ctx, p); self.write(ctx, c)
            self.t.save_index(ctx, [p, c])
            errors, _ = self.t.validate(ctx)
            self.assertTrue(any("#1" in e and "derived" in e for e in errors))

    def test_pinned_parent_off_its_rollup_is_allowed(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            p = self.base(id=1, slug="p", status="done", manual_status=True)
            c = self.base(id=2, slug="c", parent=1, status="backlog")
            self.write(ctx, p); self.write(ctx, c)
            self.t.save_index(ctx, [p, c])
            errors, _ = self.t.validate(ctx)
            self.assertEqual(errors, [])  # explicit override is exempt

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

    def test_parent_behind_its_completed_children_is_error(self):
        # an unpinned parent left `ongoing` while all its children are `done` now
        # violates the rollup invariant (#67) — it should be `done`.
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            epic = self.base(id=1, slug="e", kind="epic", status="ongoing")
            child = self.base(id=2, slug="c", parent=1, status="done")
            self.write(ctx, epic); self.write(ctx, child)
            self.t.save_index(ctx, [epic, child])
            errors, _ = self.t.validate(ctx)
            self.assertTrue(any("#1" in e and "'done'" in e and "derived" in e
                                for e in errors))

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

    def test_effective_cycle_from_hand_edited_data_is_error(self):
        # P2 -> P1 (authored) and C1(P1) -> C2(P2): an inherited deadlock that only
        # exists once dependencies are lifted through the parent hierarchy.
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            p1 = self.base(id=1, slug="p1")
            p2 = self.base(id=2, slug="p2", depends_on=[1])
            c1 = self.base(id=3, slug="c1", parent=1, depends_on=[4])
            c2 = self.base(id=4, slug="c2", parent=2)
            for r in (p1, p2, c1, c2):
                self.write(ctx, r)
            self.t.save_index(ctx, [p1, p2, c1, c2])
            errors, _ = self.t.validate(ctx)
            cyc = [e for e in errors if "dependency cycle" in e]
            self.assertTrue(cyc)
            # the message must point at the authored edges responsible
            self.assertTrue(any("effective" in e and "#2" in e for e in cyc))

    def test_child_depends_on_parent_is_effective_cycle(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            p = self.base(id=1, slug="p")
            c = self.base(id=2, slug="c", parent=1, depends_on=[1])
            self.write(ctx, p); self.write(ctx, c)
            self.t.save_index(ctx, [p, c])
            errors, _ = self.t.validate(ctx)
            self.assertTrue(any("dependency cycle" in e for e in errors))

    def test_self_parent_reported_as_parent_cycle(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            a = self.base(id=1, slug="a", parent=1)  # points at itself
            self.write(ctx, a)
            self.t.save_index(ctx, [a])
            errors, _ = self.t.validate(ctx)
            self.assertIn("parent cycle: #1 -> #1", errors)

    def test_two_node_parent_cycle_reported_once(self):
        with TemporaryDirectory() as tmp:
            ctx = self.ctx(tmp)
            a = self.base(id=1, slug="a", parent=2)
            b = self.base(id=2, slug="b", parent=1)
            self.write(ctx, a); self.write(ctx, b)
            self.t.save_index(ctx, [a, b])
            errors, _ = self.t.validate(ctx)
            cyc = [e for e in errors if "parent cycle" in e]
            self.assertEqual(len(cyc), 1)  # one error per cycle, not one per node

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


class TestRedundantDependencies(unittest.TestCase):
    """`deps` hides an edge implied by a longer path; on its own that papers over
    sloppy data forever, and the index accumulates cruft that only bites when someone
    later removes the covering edge and a hidden constraint silently reappears. So
    `check` names them — as warnings, since a redundant edge is untidy, not invalid."""

    def setUp(self):
        self.t = load_trck()

    def ctx(self, tmp, config=None):
        d = make_tracker(tmp, config or {})
        return self.t.Ctx(d, self.t.load_config(d))

    def issue(self, iid, depends=None, parent=None):
        return self.t.Issue(id=iid, slug=f"i{iid}", title=f"Item {iid}", kind="task",
                            status="backlog", priority="high", parent=parent,
                            depends_on=list(depends or []))

    def run_validate(self, tmp, *rows):
        ctx = self.ctx(tmp)
        for r in rows:
            p = self.t.issue_path(ctx, r)
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(f"# {r.title}\n")
        self.t.save_index(ctx, list(rows))
        return self.t.validate(ctx)

    def redundancy_warnings(self, warnings):
        return [w for w in warnings if "implied" in w]

    def test_an_implied_edge_is_reported(self):
        with TemporaryDirectory() as tmp:
            errors, warnings = self.run_validate(
                tmp, self.issue("a", depends=["b", "c"]),
                self.issue("b", depends=["c"]), self.issue("c"))
            found = self.redundancy_warnings(warnings)
            self.assertEqual(len(found), 1)
            self.assertIn("#a", found[0])
            self.assertIn("#c", found[0])
            self.assertIn("#b", found[0])        # names the covering path
            self.assertEqual(errors, [])         # a warning never fails the check

    def test_the_message_spells_out_the_fix(self):
        with TemporaryDirectory() as tmp:
            _errors, warnings = self.run_validate(
                tmp, self.issue("a", depends=["b", "c"]),
                self.issue("b", depends=["c"]), self.issue("c"))
            self.assertIn("dep a --remove c", self.redundancy_warnings(warnings)[0])

    def test_a_lean_graph_is_quiet(self):
        with TemporaryDirectory() as tmp:
            _errors, warnings = self.run_validate(
                tmp, self.issue("a", depends=["b"]), self.issue("b", depends=["c"]),
                self.issue("c"))
            self.assertEqual(self.redundancy_warnings(warnings), [])

    def test_a_diamond_is_quiet(self):
        with TemporaryDirectory() as tmp:
            _errors, warnings = self.run_validate(
                tmp, self.issue("a", depends=["b", "c"]), self.issue("b", depends=["d"]),
                self.issue("c", depends=["d"]), self.issue("d"))
            self.assertEqual(self.redundancy_warnings(warnings), [])

    def test_depending_on_an_epic_and_its_child_is_reported(self):
        # depending on a parent already means depending on everything it contains,
        # so the edge to the child is genuinely redundant — reached only through a
        # containment edge, which is why the check graph includes them
        with TemporaryDirectory() as tmp:
            _errors, warnings = self.run_validate(
                tmp, self.issue("a", depends=["p", "c"]), self.issue("p"),
                self.issue("c", parent="p"))
            found = self.redundancy_warnings(warnings)
            self.assertEqual(len(found), 1)
            self.assertIn("dep a --remove c", found[0])

    def test_a_parent_authored_edge_is_never_reported(self):
        # every child inherits it, so over the fully lifted graph p -> x looks
        # implied by p -> child -> x. Advising its removal would demote exactly the
        # parent-altitude edges the docs tell you to prefer, so the check graph
        # deliberately leaves inheritance out.
        with TemporaryDirectory() as tmp:
            _errors, warnings = self.run_validate(
                tmp, self.issue("p", depends=["x"]), self.issue("1", parent="p"),
                self.issue("2", parent="p"), self.issue("x"))
            self.assertEqual(self.redundancy_warnings(warnings), [])

    def test_a_containment_edge_is_never_reported(self):
        # nobody authored it, so there is nothing to remove
        with TemporaryDirectory() as tmp:
            _errors, warnings = self.run_validate(
                tmp, self.issue("p"), self.issue("1", parent="p"),
                self.issue("2", parent="p", depends=["1"]))
            for w in self.redundancy_warnings(warnings):
                self.assertNotIn("--remove 1", w)
                self.assertNotIn("--remove 2", w)

    def test_removing_the_edge_silences_the_warning(self):
        with TemporaryDirectory() as tmp:
            _errors, warnings = self.run_validate(
                tmp, self.issue("a", depends=["b"]), self.issue("b", depends=["c"]),
                self.issue("c"))
            self.assertEqual(self.redundancy_warnings(warnings), [])

    def test_a_done_issues_redundant_edge_is_not_reported(self):
        # its edges constrain nothing any more, exactly as `blocks` goes quiet for a
        # terminal row. Reporting it would make the warning permanent on historical
        # work, and a warning nobody acts on teaches people to skip warnings.
        with TemporaryDirectory() as tmp:
            a = self.issue("a", depends=["b", "c"])
            a.status = "done"
            _errors, warnings = self.run_validate(
                tmp, a, self.issue("b", depends=["c"]), self.issue("c"))
            self.assertEqual(self.redundancy_warnings(warnings), [])

    def test_an_open_issue_pointing_at_done_work_is_still_reported(self):
        # the gate is the *source* being finished, not the target: an open issue's
        # own edge list is still worth tidying
        with TemporaryDirectory() as tmp:
            b, c = self.issue("b", depends=["c"]), self.issue("c")
            b.status = c.status = "done"
            _errors, warnings = self.run_validate(tmp, self.issue("a", depends=["b", "c"]), b, c)
            self.assertEqual(len(self.redundancy_warnings(warnings)), 1)
