"""Random id generation, prefix/alias resolution, and the renumber migration (#65)."""
import tempfile
import unittest
from unittest import mock

from tests.helpers import load_trck, make_tracker, ns


class TestGenId(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def _ctx(self):
        d = make_tracker(tempfile.mkdtemp())
        return self.t.Ctx(d, self.t.load_config(d))

    def test_generated_id_matches_alphabet_and_length(self):
        t = self.t
        gid = t.gen_id(self._ctx())
        self.assertEqual(len(gid), t.ID_LEN)
        self.assertTrue(t.ID_RE.match(gid))
        self.assertFalse(set(gid) & set("01oOlI"))   # ambiguous chars excluded

    def test_within_branch_guard_redraws_on_collision(self):
        t = self.t
        ctx = self._ctx()
        # First draw collides with an existing id, second is fresh.
        with mock.patch.object(t, "_existing_ids", return_value={"aaaaaaa"}):
            with mock.patch.object(t.secrets, "choice",
                                   side_effect=list("aaaaaaa") + list("bbbbbbb")):
                gid = t.gen_id(ctx)
        self.assertEqual(gid, "bbbbbbb")


class TestResolveRef(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()
        mk = lambda i: self.t.Issue(id=i, slug="s", title="T", kind="task",
                                    status="backlog", priority="high")
        self.rows = [mk("k3m9x2a"), mk("k7zzzzz"), mk("p4abcde")]

    def test_exact_id_wins(self):
        self.assertEqual(self.t.resolve_ref(self.rows, "p4abcde").id, "p4abcde")

    def test_unique_prefix_resolves(self):
        self.assertEqual(self.t.resolve_ref(self.rows, "p4").id, "p4abcde")

    def test_ambiguous_prefix_dies(self):
        with self.assertRaises(SystemExit):
            self.t.resolve_ref(self.rows, "k")     # matches k3m9x2a and k7zzzzz

    def test_no_match_dies(self):
        with self.assertRaises(SystemExit):
            self.t.resolve_ref(self.rows, "zzz")

    def test_legacy_id_alias_resolves(self):
        t = self.t
        rows = [t.Issue(id="k3m9x2a", slug="s", title="T", kind="task",
                        status="backlog", priority="high", legacy_id=65)]
        self.assertEqual(t.resolve_ref(rows, "65").id, "k3m9x2a")
        self.assertEqual(t.resolve_ref(rows, 65).id, "k3m9x2a")

    def test_legacy_id_alias_beats_prefix(self):
        # a numeric token is read as the historical reference, not a prefix hit
        t = self.t
        rows = [t.Issue(id="65abcde", slug="s", title="T", kind="task",
                        status="backlog", priority="high"),
                t.Issue(id="k3m9x2a", slug="s2", title="T2", kind="task",
                        status="backlog", priority="high", legacy_id=65)]
        self.assertEqual(t.resolve_ref(rows, "65").id, "k3m9x2a")


class TestMergeAndOrder(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_two_branch_rows_union_without_clash(self):
        # Two independently-generated ids never collide structurally: a dict keyed
        # by id keeps both rows with intact cross-references.
        t = self.t
        a = t.Issue(id="k3m9x2a", slug="a", title="A", kind="task",
                    status="backlog", priority="high")
        b = t.Issue(id="p4abcde", slug="b", title="B", kind="task",
                    status="backlog", priority="high", depends_on=["k3m9x2a"])
        by_id = {r.id: r for r in [a, b]}
        self.assertEqual(set(by_id), {"k3m9x2a", "p4abcde"})
        self.assertEqual(by_id["p4abcde"].depends_on, ["k3m9x2a"])

    def test_list_default_sort_is_created(self):
        # When --sort is unset, the parser leaves it None and cmd_list falls back
        # to "created" order.
        t = self.t
        self.assertIsNone(t.build_parser().parse_args(["list"]).sort)


class TestRenumber(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def _tracker_with_int_ids(self):
        import tempfile
        t = self.t
        d = make_tracker(tempfile.mkdtemp())
        ctx = t.Ctx(d, t.load_config(d))
        parent = t.Issue(id="1", slug="epic", title="Epic", kind="epic",
                         status="backlog", priority="high", created=t.now_utc())
        child = t.Issue(id="2", slug="task", title="Task", kind="task",
                        status="backlog", priority="high", parent="1",
                        depends_on=[], created=t.now_utc())
        for r in (parent, child):
            p = t.issue_path(ctx, r)
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(t.TEMPLATE.format(title=r.title))
        t.save_index(ctx, [parent, child])
        return t, ctx

    def test_renumber_assigns_random_ids_and_rewrites_crossrefs(self):
        t, ctx = self._tracker_with_int_ids()
        t.cmd_renumber(ns(dir=str(ctx.dir)))
        rows = {r.legacy_id: r for r in t.load_index(ctx)}
        self.assertTrue(t.ID_RE.match(rows[1].id))
        self.assertTrue(t.ID_RE.match(rows[2].id))
        self.assertEqual(rows[2].parent, rows[1].id)          # parent rewritten
        self.assertEqual(rows[1].legacy_id, 1)
        self.assertTrue(t.issue_path(ctx, rows[1]).exists())   # file renamed to new id

    def test_renumber_is_idempotent(self):
        t, ctx = self._tracker_with_int_ids()
        t.cmd_renumber(ns(dir=str(ctx.dir)))
        first = sorted(r.id for r in t.load_index(ctx))
        t.cmd_renumber(ns(dir=str(ctx.dir)))                  # no numeric ids left
        self.assertEqual(sorted(r.id for r in t.load_index(ctx)), first)

    def test_renumber_leaves_existing_random_ids_untouched(self):
        t, ctx = self._tracker_with_int_ids()
        r = t.Issue(id="k3m9x2a", slug="r", title="R", kind="task",
                    status="backlog", priority="high", created=t.now_utc())
        p = t.issue_path(ctx, r); p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(t.TEMPLATE.format(title="R"))
        rows = t.load_index(ctx); rows.append(r); t.save_index(ctx, rows)
        t.cmd_renumber(ns(dir=str(ctx.dir)))
        ids = {x.id for x in t.load_index(ctx)}
        self.assertIn("k3m9x2a", ids)
