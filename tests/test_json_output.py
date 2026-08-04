"""`--json` on the read commands, and the one seam they all go through.

The point of a shared `emit_json` is that a consumer parsing `list --json` and
`show --json` should not have to notice which command produced the bytes: same
encoder options, same trailing newline, same issue shape from `Issue.to_dict()`.
Without a seam that drifts one command at a time and nothing catches it."""
import io
import json
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class JsonBase(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def seed(self, d, title, **over):
        args = ns(dir=str(d), title=title, priority=over.pop("priority", "medium"),
                  points=over.pop("points", None), parent=over.pop("parent", None),
                  depends=over.pop("depends", None), spec=None, slug=None,
                  review_url=None, id=over.pop("id", None))
        with redirect_stdout(io.StringIO()):
            self.t.cmd_new(args)

    def capture(self, fn, **kw):
        buf = io.StringIO()
        with redirect_stdout(buf):
            fn(ns(**kw))
        return buf.getvalue()


class TestEmitJson(JsonBase):
    def test_it_is_indented_utf8_with_a_trailing_newline(self):
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.emit_json({"title": "café", "n": [1, 2]})
        out = buf.getvalue()
        self.assertTrue(out.endswith("\n"))
        self.assertIn("café", out)          # ensure_ascii=False, not é
        self.assertIn("\n  ", out)          # indent=2
        self.assertEqual(json.loads(out), {"title": "café", "n": [1, 2]})

    def test_it_emits_exactly_one_document(self):
        """Every consumer of these commands is `json.loads(stdout)`. Two documents on
        stdout — which `show --json` used to produce, metadata then a body separator —
        means the obvious way to consume it fails."""
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.emit_json([1, 2, 3])
        self.assertEqual(len(buf.getvalue().rstrip("\n").split("\n\n")), 1)
        json.loads(buf.getvalue())          # parses whole, not line by line

    def test_an_empty_result_is_still_a_document(self):
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.emit_json([])
        self.assertEqual(json.loads(buf.getvalue()), [])


class TestShowJson(JsonBase):
    def test_it_is_one_document_with_the_body_folded_in(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha", id="aaaaaaa")
            out = self.capture(self.t.cmd_show, dir=str(d), id="aaaaaaa", json=True)
            doc = json.loads(out)           # parses whole — no `--- body ---` trailer
            self.assertEqual(doc["id"], "aaaaaaa")
            self.assertEqual(doc["title"], "Alpha")
            self.assertIn("# Alpha", doc["body"])
            self.assertNotIn("--- body ---", out)

    def test_the_body_is_the_raw_file_contents(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha", id="aaaaaaa")
            path = next((Path(d) / "items").glob("aaaaaaa-*.md"))
            path.write_text("# Alpha\n\nHand-written *prose*, `code`, and a #hash.\n")
            doc = json.loads(self.capture(self.t.cmd_show, dir=str(d), id="aaaaaaa",
                                          json=True))
            self.assertEqual(doc["body"], path.read_text())

    def test_a_non_leaf_omits_points_as_the_human_view_does(self):
        """Points roll up from leaves, so on a parent the stored value is not an input.
        The human view hides it; the JSON has to agree or the two disagree about what
        the issue *has*."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", id="aaaaaaa")
            self.seed(d, "Child", id="bbbbbbb", parent="aaaaaaa")
            epic = json.loads(self.capture(self.t.cmd_show, dir=str(d), id="aaaaaaa",
                                           json=True))
            kid = json.loads(self.capture(self.t.cmd_show, dir=str(d), id="bbbbbbb",
                                          json=True))
            self.assertNotIn("points", epic)
            self.assertIn("points", kid)

    def test_custom_fields_come_through(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha", id="aaaaaaa")
            with redirect_stdout(io.StringIO()):
                self.t.cmd_set(ns(dir=str(d), id="aaaaaaa", priority=None, points=None,
                                  parent=None, title=None, slug=None, spec=None,
                                  status=None, auto=False, review_url=None,
                                  field=["assignee=alice"], unset=None))
            doc = json.loads(self.capture(self.t.cmd_show, dir=str(d), id="aaaaaaa",
                                          json=True))
            self.assertEqual(doc["assignee"], "alice")

    def test_the_human_output_is_untouched(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha", id="aaaaaaa")
            out = self.capture(self.t.cmd_show, dir=str(d), id="aaaaaaa", json=False)
            self.assertIn("--- body ---", out)
            self.assertIn("aaaaaaa", out)


class TestDepsJson(JsonBase):
    """`{requires, blocks}` — the two cones `deps` already computes for the gutter,
    as data. Both keys are always present, empty when scoped away: a stable shape
    beats a minimal one when something is going to index into it."""

    def chain(self, d):
        # a -> b -> c, plus an unrelated island
        self.seed(d, "A", id="aaaaaaa")
        self.seed(d, "B", id="bbbbbbb", depends="aaaaaaa")
        self.seed(d, "C", id="ccccccc", depends="bbbbbbb")
        self.seed(d, "Island", id="ddddddd")

    def deps(self, d, **over):
        kw = dict(dir=str(d), id=None, requires=False, blocks=False, full=False,
                  omit_done=False, include_done_chains=False, fanout=False, json=True)
        kw.update(over)
        return json.loads(self.capture(self.t.cmd_deps, **kw))

    def ids(self, entries):
        return [e["id"] for e in entries]

    def test_both_cones_from_the_middle_of_a_chain(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.chain(d)
            doc = self.deps(d, id="bbbbbbb")
            self.assertEqual(self.ids(doc["requires"]), ["aaaaaaa"])
            self.assertEqual(self.ids(doc["blocks"]), ["ccccccc"])

    def test_the_cones_are_transitive(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.chain(d)
            self.assertEqual(self.ids(self.deps(d, id="ccccccc")["requires"]),
                             ["aaaaaaa", "bbbbbbb"])
            self.assertEqual(self.ids(self.deps(d, id="aaaaaaa")["blocks"]),
                             ["bbbbbbb", "ccccccc"])

    def test_the_focal_issue_is_not_in_its_own_cones(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.chain(d)
            doc = self.deps(d, id="bbbbbbb")
            self.assertNotIn("bbbbbbb", self.ids(doc["requires"]) + self.ids(doc["blocks"]))

    def test_scoping_empties_the_other_key_rather_than_dropping_it(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.chain(d)
            up = self.deps(d, id="bbbbbbb", requires=True)
            self.assertEqual(self.ids(up["requires"]), ["aaaaaaa"])
            self.assertEqual(up["blocks"], [])
            down = self.deps(d, id="bbbbbbb", blocks=True)
            self.assertEqual(up.keys(), down.keys())
            self.assertEqual(down["requires"], [])

    def test_an_issue_with_no_edges_gives_two_empty_lists(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.chain(d)
            self.assertEqual(self.deps(d, id="ddddddd"), {"requires": [], "blocks": []})

    def test_containment_is_followed_like_the_gutter_does(self):
        """A parent is waiting on its children and a child is contained by its parent,
        so the cones cross hierarchy edges — same rule the human graph draws."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Epic", id="aaaaaaa")
            self.seed(d, "Kid", id="bbbbbbb", parent="aaaaaaa")
            self.assertEqual(self.ids(self.deps(d, id="aaaaaaa")["requires"]), ["bbbbbbb"])
            self.assertEqual(self.ids(self.deps(d, id="bbbbbbb")["blocks"]), ["aaaaaaa"])

    def test_entries_are_whole_issue_objects(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.chain(d)
            entry = self.deps(d, id="bbbbbbb")["requires"][0]
            for k in ("id", "slug", "title", "status", "priority"):
                self.assertIn(k, entry)
            self.assertEqual(entry["title"], "A")

    def test_the_order_is_deterministic(self):
        """Cones are computed as sets. Emitting them in set order would make the output
        depend on hash seeding, which a golden file cannot survive."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.chain(d)
            self.seed(d, "D", id="eeeeeee", depends="aaaaaaa")
            self.seed(d, "E", id="fffffff", depends="aaaaaaa")
            first = self.ids(self.deps(d, id="aaaaaaa")["blocks"])
            self.assertEqual(first, sorted(first, key=lambda i: first.index(i)))
            for _ in range(3):
                self.assertEqual(self.ids(self.deps(d, id="aaaaaaa")["blocks"]), first)

    def test_it_needs_an_id(self):
        """Without one the human graph draws every component, which is a different
        shape — an edge list, not a pair of cones. Emitting two schemas from one flag
        would make every consumer branch on whether an id was passed."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.chain(d)
            with self.assertRaises(SystemExit):
                self.capture(self.t.cmd_deps, dir=str(d), id=None, requires=False,
                             blocks=False, full=False, omit_done=False,
                             include_done_chains=False, fanout=False, json=True)

    def test_the_human_graph_is_untouched(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.chain(d)
            out = self.capture(self.t.cmd_deps, dir=str(d), id="bbbbbbb",
                               requires=False, blocks=False, full=False,
                               omit_done=False, include_done_chains=False,
                               fanout=False, json=False)
            self.assertIn("#aaaaaaa", out)
            self.assertIn("●", out)


class TestListJson(JsonBase):
    """Nested by default, mirroring the on-screen forest; `--flat --json` is the flat
    array. Both honour every filter and sort exactly as the human render does — the
    JSON is the same query, differently written out."""

    def tree(self, d):
        self.seed(d, "Epic", id="aaaaaaa")
        self.seed(d, "Kid one", id="bbbbbbb", parent="aaaaaaa")
        self.seed(d, "Kid two", id="ccccccc", parent="aaaaaaa")
        self.seed(d, "Grandkid", id="ddddddd", parent="bbbbbbb")
        self.seed(d, "Loose", id="eeeeeee")

    def listing(self, d, **over):
        kw = dict(dir=str(d), status=None, priority=None, label=None, parent=None,
                  match=None, sort=None, blocked=False, orphan=False, all=False,
                  flat=False, paths=False, id=None, field=None, show_field=None,
                  json=True)
        kw.update(over)
        return json.loads(self.capture(self.t.cmd_list, **kw))

    def test_the_default_is_a_nested_forest(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.tree(d)
            doc = self.listing(d)
            self.assertEqual([r["id"] for r in doc], ["aaaaaaa", "eeeeeee"])
            epic = doc[0]
            self.assertEqual([c["id"] for c in epic["children"]],
                             ["bbbbbbb", "ccccccc"])
            self.assertEqual([g["id"] for g in epic["children"][0]["children"]],
                             ["ddddddd"])

    def test_a_leaf_still_carries_an_empty_children_list(self):
        """Always present, so a consumer can recurse without checking for the key."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.tree(d)
            self.assertEqual(self.listing(d)[1]["children"], [])

    def test_flat_is_a_flat_array_in_the_sorted_order(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Low", id="aaaaaaa", priority="low")
            self.seed(d, "Urgent", id="bbbbbbb", priority="urgent")
            self.seed(d, "Mid", id="ccccccc", priority="medium")
            doc = self.listing(d, flat=True, sort="priority")
            self.assertEqual([r["id"] for r in doc],
                             ["bbbbbbb", "ccccccc", "aaaaaaa"])
            self.assertNotIn("children", doc[0])

    def test_filters_are_honoured(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.tree(d)
            doc = self.listing(d, flat=True, match="kid")
            self.assertEqual([r["id"] for r in doc],
                             ["bbbbbbb", "ccccccc", "ddddddd"])

    def test_an_empty_result_is_an_empty_array(self):
        """Not silence. A consumer doing `json.loads(stdout)` should not have to
        special-case "no matches" as a parse error."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.tree(d)
            self.assertEqual(self.listing(d, flat=True, match="nothing matches"), [])
            self.assertEqual(self.listing(d, match="nothing matches"), [])

    def test_ancestor_context_is_included_and_marked(self):
        """The forest pulls non-matching ancestors back in so a matched child never
        floats free; the human view dims them. Marking them keeps that information
        without leaking ANSI — otherwise a consumer cannot tell a match from scaffolding."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.tree(d)
            doc = self.listing(d, match="grandkid")
            self.assertEqual([r["id"] for r in doc], ["aaaaaaa"])
            self.assertTrue(doc[0]["context"])                       # Epic: scaffolding
            kid = doc[0]["children"][0]
            self.assertTrue(kid["context"])                          # Kid one: scaffolding
            self.assertFalse(kid["children"][0]["context"])           # Grandkid: the match

    def test_a_root_id_scopes_to_that_subtree(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.tree(d)
            doc = self.listing(d, id="bbbbbbb")
            self.assertEqual([r["id"] for r in doc], ["bbbbbbb"])
            self.assertEqual([c["id"] for c in doc[0]["children"]], ["ddddddd"])

    def test_ids_are_never_abbreviated(self):
        """`unique_prefix_lens` is a display concern. A shortened id in data would be
        a different value, not a shorter rendering of the same one."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.tree(d)
            for r in self.listing(d, flat=True):
                self.assertEqual(len(r["id"]), self.t.ID_LEN)

    def test_paths_and_json_together_are_refused(self):
        """Two different output modes. Silently letting one win would make a script
        that asks for both get the other."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.tree(d)
            with self.assertRaises(SystemExit):
                self.listing(d, paths=True)

    def test_the_human_output_is_untouched(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.tree(d)
            out = self.capture(self.t.cmd_list, dir=str(d), status=None, priority=None,
                               label=None, parent=None, match=None, sort=None,
                               blocked=False, orphan=False, all=False, flat=False,
                               paths=False, id=None, field=None, show_field=None,
                               json=False)
            self.assertIn("├─ ", out)
            self.assertIn("#aaaaaaa", out)


class TestReadyJson(JsonBase):
    """The rank order *is* the payload. A consumer must not have to re-derive it, so
    the array order is the contract and the demand annotation travels as fields rather
    than baked into the `↑urgent(#a1b2c3)` string the human view prints."""

    def ready(self, d, **over):
        kw = dict(dir=str(d), id=None, next=False, json=True)
        kw.update(over)
        return json.loads(self.capture(self.t.cmd_ready, **kw))

    def test_it_is_an_array_in_rank_order(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Low", id="aaaaaaa", priority="low")
            self.seed(d, "Urgent", id="bbbbbbb", priority="urgent")
            self.seed(d, "Mid", id="ccccccc", priority="medium")
            self.assertEqual([r["id"] for r in self.ready(d)],
                             ["bbbbbbb", "ccccccc", "aaaaaaa"])

    def test_blocked_issues_are_absent(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Dep", id="aaaaaaa")
            self.seed(d, "Blocked", id="bbbbbbb", depends="aaaaaaa")
            self.assertEqual([r["id"] for r in self.ready(d)], ["aaaaaaa"])

    def test_demand_travels_as_fields_not_prose(self):
        """A medium task standing between you and an urgent one outranks a high one
        that blocks nothing — and the row says why. As a string that explanation is
        unusable; as `demand_priority`/`demand_source` it is queryable."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Blocker", id="aaaaaaa", priority="medium")
            self.seed(d, "Urgent thing", id="bbbbbbb", priority="urgent",
                      depends="aaaaaaa")
            self.seed(d, "High, blocks nothing", id="ccccccc", priority="high")
            doc = self.ready(d)
            self.assertEqual([r["id"] for r in doc], ["aaaaaaa", "ccccccc"])
            self.assertEqual(doc[0]["demand_priority"], "urgent")
            self.assertEqual(doc[0]["demand_source"], "bbbbbbb")

    def test_an_unlifted_row_omits_the_demand_fields(self):
        """Most rows are their own maximum. Emitting nulls everywhere would suggest the
        fields mean something on every row; the human view prints nothing there."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alone", id="aaaaaaa", priority="high")
            row = self.ready(d)[0]
            self.assertNotIn("demand_priority", row)
            self.assertNotIn("demand_source", row)

    def test_next_is_the_same_shape_capped_at_one(self):
        """An array either way, so a consumer switching between them changes nothing
        but the length."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Low", id="aaaaaaa", priority="low")
            self.seed(d, "Urgent", id="bbbbbbb", priority="urgent")
            doc = json.loads(self.capture(self.t.cmd_next, dir=str(d), id=None,
                                          json=True))
            self.assertEqual([r["id"] for r in doc], ["bbbbbbb"])

    def test_an_empty_result_is_an_empty_array_and_exits_zero(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.assertEqual(self.ready(d), [])
            self.assertEqual(json.loads(self.capture(self.t.cmd_next, dir=str(d),
                                                     id=None, json=True)), [])

    def test_a_root_id_filters_the_result_not_the_ranking(self):
        """Readiness and rank stay computed over the whole graph — narrowing the graph
        would make work blocked from outside the subtree look actionable."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Outside", id="aaaaaaa")
            self.seed(d, "Epic", id="bbbbbbb")
            self.seed(d, "In subtree, blocked from outside", id="ccccccc",
                      parent="bbbbbbb", depends="aaaaaaa")
            self.seed(d, "In subtree, free", id="ddddddd", parent="bbbbbbb")
            self.assertEqual([r["id"] for r in self.ready(d, id="bbbbbbb")],
                             ["ddddddd"])

    def test_ids_are_never_abbreviated(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Alpha", id="aaaaaaa")
            self.assertEqual(len(self.ready(d)[0]["id"]), self.t.ID_LEN)

    def test_the_human_output_is_untouched(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "Blocker", id="aaaaaaa", priority="medium")
            self.seed(d, "Urgent thing", id="bbbbbbb", priority="urgent",
                      depends="aaaaaaa")
            out = self.capture(self.t.cmd_ready, dir=str(d), id=None, next=False,
                               json=False)
            self.assertIn("#aaaaaaa", out)
            self.assertIn("↑urgent", out)
