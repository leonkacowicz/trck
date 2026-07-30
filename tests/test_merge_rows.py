"""Row-wise 3-way merge of index.jsonl rows (#pfackaw).

Every rule is symmetric or derived from the base, because a merge driver cannot
determine which side is the user's — `%A` is whatever is checked out at that moment,
which differs between `git merge main` and `git rebase main` on the *same* branch.
So each test that asserts a merge result also asserts the mirrored call gives the
same answer.
"""
import unittest

from tests.helpers import load_trck


def row(iid="abc1234", **over):
    r = {"id": iid, "slug": "alpha", "title": "Alpha", "kind": "task",
         "status": "backlog", "priority": "medium"}
    r.update(over)
    return r


class TestMergeRows(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def merge(self, base, a, b):
        """merge_rows(base, a, b) -> (rows, conflicts). Asserts symmetry."""
        rows_ab, conf_ab = self.t.merge_rows(base, a, b)
        rows_ba, conf_ba = self.t.merge_rows(base, b, a)
        key = lambda rs: sorted(json_of(r) for r in rs)
        self.assertEqual(key(rows_ab), key(rows_ba),
                         "merge is not symmetric in its two sides")
        self.assertEqual(sorted(conf_ab), sorted(conf_ba),
                         "conflict set is not symmetric in its two sides")
        return rows_ab, conf_ab

    # --- disjoint creation: the easy case that must never conflict ----------- #

    def test_each_side_adds_a_new_issue(self):
        base = []
        a = [row("aaa1111")]
        b = [row("bbb2222")]
        rows, conflicts = self.merge(base, a, b)
        self.assertEqual(sorted(r.id for r in rows), ["aaa1111", "bbb2222"])
        self.assertEqual(conflicts, [])

    def test_row_deleted_on_one_side_stays_deleted(self):
        base = [row("aaa1111"), row("bbb2222")]
        a = [row("aaa1111")]                 # bbb2222 removed
        b = [row("aaa1111"), row("bbb2222")]  # untouched
        rows, conflicts = self.merge(base, a, b)
        self.assertEqual([r.id for r in rows], ["aaa1111"])
        self.assertEqual(conflicts, [])

    # --- independent fields on the same row ---------------------------------- #

    def test_independent_scalar_edits_both_apply(self):
        base = [row(priority="medium", title="Alpha")]
        a = [row(priority="urgent", title="Alpha")]
        b = [row(priority="medium", title="Renamed")]
        rows, conflicts = self.merge(base, a, b)
        self.assertEqual(conflicts, [])
        self.assertEqual(rows[0].priority, "urgent")
        self.assertEqual(rows[0].title, "Renamed")

    def test_same_scalar_changed_differently_conflicts(self):
        base = [row(priority="medium")]
        a = [row(priority="urgent")]
        b = [row(priority="low")]
        _, conflicts = self.merge(base, a, b)
        self.assertTrue(any("priority" in c for c in conflicts), conflicts)

    def test_same_scalar_changed_identically_is_not_a_conflict(self):
        base = [row(priority="medium")]
        a = [row(priority="urgent")]
        b = [row(priority="urgent")]
        rows, conflicts = self.merge(base, a, b)
        self.assertEqual(conflicts, [])
        self.assertEqual(rows[0].priority, "urgent")

    # --- set-valued fields union --------------------------------------------- #

    def test_labels_union_across_both_sides(self):
        base = [row(labels=["keep"])]
        a = [row(labels=["keep", "from-a"])]
        b = [row(labels=["keep", "from-b"])]
        rows, conflicts = self.merge(base, a, b)
        self.assertEqual(conflicts, [])
        self.assertEqual(rows[0].labels, ["from-a", "from-b", "keep"])

    def test_depends_on_unions_across_both_sides(self):
        base = [row(depends_on=[]), row("d1"), row("d2")]
        a = [row(depends_on=["d1"]), row("d1"), row("d2")]
        b = [row(depends_on=["d2"]), row("d1"), row("d2")]
        rows, conflicts = self.merge(base, a, b)
        self.assertEqual(conflicts, [])
        merged = next(r for r in rows if r.id == "abc1234")
        self.assertEqual(merged.depends_on, ["d1", "d2"])

    def test_a_label_removed_on_one_side_stays_removed(self):
        """Union alone would resurrect it; the base says it was deliberately dropped."""
        base = [row(labels=["old", "keep"])]
        a = [row(labels=["keep"])]              # removed 'old'
        b = [row(labels=["old", "keep"])]       # untouched
        rows, conflicts = self.merge(base, a, b)
        self.assertEqual(conflicts, [])
        self.assertEqual(rows[0].labels, ["keep"])

    # --- monotone timestamps -------------------------------------------------- #

    def test_created_takes_the_earliest(self):
        base = [row(created="2026-01-02T00:00:00Z")]
        a = [row(created="2026-01-01T00:00:00Z")]
        b = [row(created="2026-01-03T00:00:00Z")]
        rows, _ = self.merge(base, a, b)
        self.assertEqual(rows[0].created, "2026-01-01T00:00:00Z")

    # --- the lifecycle tuple, merged atomically ------------------------------- #

    def test_one_side_moves_status_the_other_leaves_it(self):
        base = [row(status="backlog")]
        a = [row(status="ongoing", started="2026-01-01T00:00:00Z")]
        b = [row(status="backlog")]
        rows, conflicts = self.merge(base, a, b)
        self.assertEqual(conflicts, [])
        self.assertEqual(rows[0].status, "ongoing")
        self.assertEqual(rows[0].started, "2026-01-01T00:00:00Z")

    def test_both_sides_move_status_conflicts(self):
        base = [row(status="backlog")]
        a = [row(status="ongoing")]
        b = [row(status="done", closed="2026-01-01T00:00:00Z")]
        _, conflicts = self.merge(base, a, b)
        self.assertTrue(any("abc1234" in c for c in conflicts), conflicts)

    def test_tuple_is_atomic_the_documented_corruption_case(self):
        """The #ey2aruc example: one side sets resolution without moving status,
        the other reopens. Field-wise this looks clean and yields a row no verb can
        write (status=ongoing + resolution=wontfix). It must conflict instead."""
        base = [row(status="done", closed="T1", resolution=None)]
        a = [row(status="done", closed="T1", resolution="wontfix")]
        b = [row(status="ongoing", closed=None, resolution=None)]
        rows, conflicts = self.merge(base, a, b)
        self.assertTrue(conflicts, "the tuple must conflict, not merge field-wise")
        for r in rows:
            self.assertFalse(r.status == "ongoing" and r.resolution == "wontfix",
                             "produced the exact corrupt row the design forbids")

    def test_tuple_untouched_by_either_side_is_left_alone(self):
        base = [row(status="done", closed="T1", resolution="wontfix")]
        a = [row(status="done", closed="T1", resolution="wontfix", priority="low")]
        b = [row(status="done", closed="T1", resolution="wontfix", title="Renamed")]
        rows, conflicts = self.merge(base, a, b)
        self.assertEqual(conflicts, [])
        self.assertEqual(rows[0].status, "done")
        self.assertEqual(rows[0].resolution, "wontfix")

    def test_status_change_on_one_side_and_scalar_on_the_other_both_apply(self):
        base = [row(status="backlog", priority="medium")]
        a = [row(status="done", closed="T1")]
        b = [row(status="backlog", priority="urgent")]
        rows, conflicts = self.merge(base, a, b)
        self.assertEqual(conflicts, [])
        self.assertEqual(rows[0].status, "done")
        self.assertEqual(rows[0].priority, "urgent")

    # --- derived fields on non-leaves are never merged ------------------------ #

    def test_a_parents_status_never_conflicts(self):
        """A parent's status is derived from its children by `normalize_statuses`,
        so a divergence there is not a real disagreement — it is two sides having
        recomputed from different child sets. Recompute after the merge instead of
        conflicting. The children themselves still merge normally."""
        base = [row("par1111", status="backlog"),
                row("kid1111", status="backlog", parent="par1111")]
        a = [row("par1111", status="ongoing"),
             row("kid1111", status="backlog", parent="par1111")]
        b = [row("par1111", status="done", closed="T1"),
             row("kid1111", status="backlog", parent="par1111")]
        _, conflicts = self.merge(base, a, b)
        self.assertEqual([c for c in conflicts if "par1111" in c], [],
                         f"parent status must not conflict: {conflicts}")

    def test_a_leaf_status_divergence_still_conflicts(self):
        """The guard above must not silently disable the leaf rule."""
        base = [row("kid1111", status="backlog")]
        a = [row("kid1111", status="ongoing")]
        b = [row("kid1111", status="done", closed="T1")]
        _, conflicts = self.merge(base, a, b)
        self.assertTrue(any("kid1111" in c for c in conflicts), conflicts)

    # --- conflict reporting --------------------------------------------------- #

    def test_conflict_messages_never_say_ours_or_theirs(self):
        base = [row(priority="medium", status="backlog")]
        a = [row(priority="urgent", status="ongoing")]
        b = [row(priority="low", status="done")]
        _, conflicts = self.merge(base, a, b)
        self.assertTrue(conflicts)
        for c in conflicts:
            low = c.lower()
            for word in ("ours", "theirs", "yours", "mine"):
                self.assertNotIn(word, low, f"orientation word in: {c}")

    def test_conflict_message_names_both_values(self):
        base = [row(priority="medium")]
        a = [row(priority="urgent")]
        b = [row(priority="low")]
        _, conflicts = self.merge(base, a, b)
        joined = " ".join(conflicts)
        self.assertIn("urgent", joined)
        self.assertIn("low", joined)


def json_of(r):
    import json
    return json.dumps(r.to_canonical(), sort_keys=True)
