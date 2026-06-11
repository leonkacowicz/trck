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
            self.assertEqual(line["assignee"], "leon")
            self.assertEqual(line["component"], "ui")
            keys = list(line)
            self.assertLess(keys.index("assignee"), keys.index("component"))

    def test_field_missing_equals_rejected(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            with self.assertRaises(SystemExit):
                self.set_(d, 1, field=["justkey"])

    def test_unset_bad_key_rejected(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d)
            with self.assertRaises(SystemExit):
                self.set_(d, 1, unset=["status"])

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

    # -- list --field filter -------------------------------------------------
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
