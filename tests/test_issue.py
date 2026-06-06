"""Unit tests for the Issue dataclass: construction defaults, dict (de)serialization,
canonical slim form, and legacy migration. The Issue type is the single in-memory
representation of an issue; index.jsonl round-trips through from_dict / to_canonical."""
import dataclasses
import unittest

from tests.helpers import load_trck


class TestIssueModel(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        self.Issue = self.t.Issue

    def minimal(self, **over):
        base = dict(id=1, slug="a", title="A", kind="task",
                    status="backlog", priority="high")
        base.update(over)
        return self.Issue(**base)

    # -- construction / defaults --------------------------------------------
    def test_is_a_dataclass(self):
        self.assertTrue(dataclasses.is_dataclass(self.Issue))

    def test_optional_fields_have_canonical_defaults(self):
        i = self.minimal()
        self.assertEqual(i.points, 1)
        self.assertIsNone(i.parent)
        self.assertEqual(i.labels, [])
        self.assertEqual(i.depends_on, [])
        self.assertIsNone(i.spec)
        self.assertIsNone(i.created)
        self.assertIsNone(i.started)
        self.assertIsNone(i.closed)
        self.assertIsNone(i.resolution)
        self.assertEqual(i.extra, {})

    def test_mutable_defaults_are_not_shared(self):
        a, b = self.minimal(), self.minimal()
        a.labels.append("x")
        a.depends_on.append(2)
        a.extra["k"] = 1
        self.assertEqual(b.labels, [])
        self.assertEqual(b.depends_on, [])
        self.assertEqual(b.extra, {})

    # -- from_dict ----------------------------------------------------------
    def test_from_dict_maps_known_keys_to_attributes(self):
        i = self.Issue.from_dict({"id": 7, "slug": "s", "title": "T", "kind": "task",
                                  "status": "done", "priority": "low", "parent": 3,
                                  "labels": ["x"], "depends_on": [1, 2]})
        self.assertEqual(i.id, 7)
        self.assertEqual(i.parent, 3)
        self.assertEqual(i.labels, ["x"])
        self.assertEqual(i.depends_on, [1, 2])

    def test_from_dict_routes_unknown_keys_to_extra(self):
        i = self.Issue.from_dict({"id": 1, "slug": "a", "title": "A", "kind": "task",
                                  "status": "backlog", "priority": "high",
                                  "zeta": 9, "vendor": None})
        self.assertEqual(i.extra, {"zeta": 9, "vendor": None})

    def test_from_dict_tolerates_missing_fields(self):
        i = self.Issue.from_dict({"id": 1})  # malformed row; validate reports, ctor must not crash
        self.assertEqual(i.id, 1)
        self.assertIsNone(i.status)
        self.assertEqual(i.labels, [])

    def test_from_dict_migrates_milestone_to_label(self):
        i = self.Issue.from_dict({"id": 1, "slug": "a", "title": "A", "kind": "task",
                                  "status": "backlog", "priority": "high",
                                  "milestone": "v1.0"})
        self.assertEqual(i.labels, ["v1.0"])
        self.assertNotIn("milestone", i.extra)

    def test_from_dict_drops_null_milestone(self):
        i = self.Issue.from_dict({"id": 1, "slug": "a", "title": "A", "kind": "task",
                                  "status": "backlog", "priority": "high",
                                  "milestone": None})
        self.assertEqual(i.labels, [])
        self.assertNotIn("milestone", i.extra)

    # -- to_canonical (slim, ordered serialization) -------------------------
    def test_to_canonical_strips_fields_equal_to_default(self):
        i = self.minimal(created="2026-06-05")
        obj = i.to_canonical()
        for stripped in ("points", "parent", "labels", "depends_on", "spec",
                         "started", "closed", "resolution"):
            self.assertNotIn(stripped, obj)
        self.assertEqual(list(obj.keys()),
                         ["id", "slug", "title", "kind", "status", "priority", "created"])

    def test_to_canonical_keeps_non_default_in_canon_order(self):
        i = self.minimal(status="done", priority="low", parent=1, labels=["m1"],
                         depends_on=[1], created="2026-06-05", resolution="superseded")
        obj = i.to_canonical()
        self.assertEqual(list(obj.keys()),
                         ["id", "slug", "title", "kind", "status", "priority",
                          "parent", "labels", "depends_on", "created", "resolution"])

    def test_to_canonical_omits_none_required_field(self):
        i = self.minimal()  # no created
        self.assertNotIn("created", i.to_canonical())

    def test_to_canonical_appends_extra_keys_sorted_last(self):
        i = self.minimal(extra={"zeta": 1, "alpha": 2})
        keys = list(i.to_canonical().keys())
        self.assertEqual(keys[:3], ["id", "slug", "title"])
        self.assertEqual(keys[-2:], ["alpha", "zeta"])

    def test_to_canonical_keeps_custom_keys_even_when_empty(self):
        i = self.minimal(extra={"custom_null": None, "custom_empty": [], "custom_str": ""})
        obj = i.to_canonical()
        self.assertIsNone(obj["custom_null"])
        self.assertEqual(obj["custom_empty"], [])
        self.assertEqual(obj["custom_str"], "")

    # -- round-trip ---------------------------------------------------------
    def test_round_trip_through_canonical_is_stable(self):
        i = self.minimal(status="done", priority="low", parent=1, labels=["m1"],
                         depends_on=[1], created="2026-06-05", resolution="superseded",
                         extra={"vendor": None})
        again = self.Issue.from_dict(i.to_canonical())
        self.assertEqual(again.to_canonical(), i.to_canonical())


if __name__ == "__main__":
    unittest.main()
