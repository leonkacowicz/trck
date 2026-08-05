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
                  priority=over.pop("priority", "high"),
                  parent=None, depends=None, spec=None, slug=None, points=None)
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(args)
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def set_(self, d, iid, **over):
        args = ns(dir=str(d), id=iid, priority=None, points=None, parent=None,
                  spec=None, title=None, slug=None,
                  field=over.pop("field", None), unset=over.pop("unset", None))
        self.t.cmd_set(args)

    def test_reserved_key_rejected(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            with self.assertRaises(SystemExit):
                self.set_(d, id1, field=["status=foo"])

    def test_malformed_key_rejected(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            for bad in ["Assignee=x", "1tag=x", "a b=x"]:
                with self.assertRaises(SystemExit):
                    self.set_(d, id1, field=[bad])

    def test_field_missing_equals_rejected(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            with self.assertRaises(SystemExit):
                self.set_(d, id1, field=["justkey"])

    def test_unset_bad_key_rejected(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            with self.assertRaises(SystemExit):
                self.set_(d, id1, unset=["status"])

    def test_check_passes_with_custom_fields(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            self.set_(d, id1, field=["assignee=alice"])
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

    # -- list --field filter -------------------------------------------------
    def list_(self, d, **over):
        args = ns(dir=str(d), id=None, flat=True, status=None,
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
            id1 = self.seed(d, title="Alpha")
            id2 = self.seed(d, title="Beta")
            self.set_(d, id1, field=["assignee=alice"])
            self.set_(d, id2, field=["assignee=mara"])
            out = self.list_(d, field=["assignee=alice"])
            self.assertIn("Alpha", out)
            self.assertNotIn("Beta", out)

    def test_field_filter_anded(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, title="Alpha")
            id2 = self.seed(d, title="Beta")
            self.set_(d, id1, field=["assignee=alice", "component=ui"])
            self.set_(d, id2, field=["assignee=alice", "component=api"])
            out = self.list_(d, field=["assignee=alice", "component=ui"])
            self.assertIn("Alpha", out)
            self.assertNotIn("Beta", out)

    def _order(self, out):
        # the leading "#<id>" of each printed row, in print order
        import re
        return re.findall(r"#([a-z0-9]+)", out)

    def test_sort_by_field_missing_last(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, title="One")    # -> zebra
            id2 = self.seed(d, title="Two")    # -> alpha
            id3 = self.seed(d, title="Three")  # -> (unset)
            self.set_(d, id1, field=["owner=zebra"])
            self.set_(d, id2, field=["owner=alpha"])
            out = self.list_(d, sort="field:owner")
            self.assertEqual(self._order(out), [id2, id1, id3])

    def test_show_field_column(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, title="Alpha")
            id2 = self.seed(d, title="Beta")
            self.set_(d, id1, field=["component=ui"])
            out = self.list_(d, show_field=["component"])
            line1 = next(l for l in out.splitlines() if f"#{id1}" in l)
            line2 = next(l for l in out.splitlines() if f"#{id2}" in l)
            self.assertIn("component=ui", line1)
            self.assertNotIn("component=", line2)

    def test_list_field_reserved_key_rejected(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            with self.assertRaises(SystemExit):
                self.list_(d, field=["status=backlog"])

    def test_show_displays_custom_field(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d)
            self.set_(d, id1, field=["assignee=alice"])
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_show(ns(dir=str(d), id=id1, json=False))
            out = buf.getvalue()
            self.assertIn("assignee", out)
            self.assertIn("alice", out)

    def test_field_filter_composes_with_status(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            id1 = self.seed(d, title="Alpha")
            id2 = self.seed(d, title="Beta")
            self.set_(d, id1, field=["assignee=alice"])
            self.set_(d, id2, field=["assignee=alice"])
            self.t.cmd_mv(ns(dir=str(d), id=id1, status="ongoing", resolution=None))
            out = self.list_(d, field=["assignee=alice"], status="ongoing")
            self.assertIn("Alpha", out)
            self.assertNotIn("Beta", out)


class TestKindIsJustAField(unittest.TestCase):
    """`kind` was a required field with a configured vocabulary and no behaviour behind
    it — a glorified key-value pair. It is now an actual one.

    The only semantics it ever carried was `epic`, and that was a declared fact
    duplicating a derived one: on this repo's own tracker three issues were marked epic
    with no children and one had children without the mark. Deriving it from the
    hierarchy makes the marker true by construction."""

    def setUp(self):
        self.t = load_trck()

    def test_kind_is_no_longer_a_canonical_field(self):
        self.assertNotIn("kind", self.t.CANON_KEYS)

    def test_an_existing_rows_kind_survives_as_a_custom_field(self):
        """No migration writes this: dropping `kind` from CANON_KEYS is enough, because
        `from_dict` routes every non-canonical key into `extra` and `to_canonical` writes
        it back out. Trackers written before this keep the value they had."""
        row = self.t.Issue.from_dict({"id": "a1b2c3d", "slug": "s", "title": "T",
                                      "status": "backlog", "priority": "medium",
                                      "kind": "bug"})
        self.assertEqual(row.extra["kind"], "bug")
        self.assertEqual(row.to_canonical()["kind"], "bug")

    def test_kind_is_not_required(self):
        row = self.t.Issue.from_dict({"id": "a1b2c3d", "slug": "s", "title": "T",
                                      "status": "backlog", "priority": "medium"})
        self.assertEqual(row.extra, {})

    def test_kind_may_be_set_as_a_plain_field(self):
        self.assertIsNone(self.t.check_field_key("kind"))

    def test_the_vocabulary_key_is_vestigial(self):
        warns = self.t.check_vestigial_vocabulary({"kinds": ["task"]})
        self.assertEqual(len(warns), 1)
        self.assertIn("no longer configurable", warns[0])
