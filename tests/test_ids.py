"""Random id generation, prefix/alias resolution, and the renumber migration (#65)."""
import io
import tempfile
import unittest
from contextlib import redirect_stdout
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

    def test_generated_id_is_never_all_digits(self):
        # all-digit ids are reserved for legacy integer ids; gen_id must avoid them
        # so the "all-digit ⇔ legacy" discriminator (renumber/filename) stays sound.
        t = self.t
        # force an all-digit draw first, then a valid one; gen_id must skip the digits
        with mock.patch.object(t, "_existing_ids", return_value=set()):
            with mock.patch.object(t.secrets, "choice",
                                   side_effect=list("2345678") + list("abcdefg")):
                gid = t.gen_id(self._ctx())
        self.assertFalse(gid.isdigit())
        self.assertEqual(gid, "abcdefg")


class TestUniquePrefixLens(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_distinguishes_at_first_differing_char(self):
        m = self.t.unique_prefix_lens(["k3m9x2a", "k7zzzzz", "p4abcde"])
        self.assertEqual(m["p4abcde"], 1)   # only id starting 'p'
        self.assertEqual(m["k3m9x2a"], 2)   # shares 'k', diverges at index 1
        self.assertEqual(m["k7zzzzz"], 2)

    def test_single_id_needs_one_char(self):
        self.assertEqual(self.t.unique_prefix_lens(["abcdefg"]), {"abcdefg": 1})

    def test_prefix_subset_falls_back_to_full_length(self):
        # "1" is a prefix of "10": no shorter unique prefix exists, so use full id
        m = self.t.unique_prefix_lens(["1", "10"])
        self.assertEqual(m["1"], 1)
        self.assertEqual(m["10"], 2)

    def test_handles_duplicates_in_input(self):
        m = self.t.unique_prefix_lens(["abc", "abc", "axy"])
        self.assertEqual(m["abc"], 2)
        self.assertEqual(m["axy"], 2)


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


class TestDepsRootResolution(unittest.TestCase):
    """`deps <id>` must resolve a prefix / legacy-id token like every other id arg,
    not compare the raw token against resolved ids (regression: a prefix wrongly
    printed '(no dependencies)')."""
    def setUp(self):
        self.t = load_trck()

    def _tracker(self):
        t = self.t
        d = make_tracker(tempfile.mkdtemp())
        ctx = t.Ctx(d, t.load_config(d))
        a = t.Issue(id="aabbcc2", slug="a", title="Prereq", kind="task",
                    status="backlog", priority="high", created=t.now_utc())
        b = t.Issue(id="ddee3f4", slug="b", title="Dependent", kind="task",
                    status="backlog", priority="high", depends_on=["aabbcc2"],
                    created=t.now_utc(), legacy_id=21)
        for r in (a, b):
            p = t.issue_path(ctx, r)
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(t.TEMPLATE.format(title=r.title))
        t.save_index(ctx, [a, b])
        return ctx

    def _deps(self, ctx, token):
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_deps(ns(dir=str(ctx.dir), id=token,
                               requires=False, blocks=False, full=False))
        return buf.getvalue()

    def test_prefix_root_shows_the_cone(self):
        ctx = self._tracker()
        out = self._deps(ctx, "ddee")          # prefix of ddee3f4
        self.assertNotIn("(no dependencies)", out)
        self.assertIn("aabbcc2", out)          # its prerequisite is drawn

    def test_legacy_alias_root_shows_the_cone(self):
        ctx = self._tracker()
        out = self._deps(ctx, "21")            # legacy_id alias for ddee3f4
        self.assertNotIn("(no dependencies)", out)
        self.assertIn("aabbcc2", out)

    def test_exact_root_still_works(self):
        ctx = self._tracker()
        out = self._deps(ctx, "ddee3f4")
        self.assertNotIn("(no dependencies)", out)
        self.assertIn("aabbcc2", out)

    def test_genuinely_depless_issue_still_reports_no_dependencies(self):
        ctx = self._tracker()
        out = self._deps(ctx, "aabbcc2")       # a has no deps and nothing depends-cone via prefix
        # a IS depended-on by b, so its down-cone is non-empty; use a 3rd isolated issue:
        t = self.t
        iso = t.Issue(id="zzz9k8m", slug="iso", title="Lonely", kind="task",
                      status="backlog", priority="high", created=t.now_utc())
        p = t.issue_path(ctx, iso); p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(t.TEMPLATE.format(title="Lonely"))
        rows = t.load_index(ctx); rows.append(iso); t.save_index(ctx, rows)
        out = self._deps(ctx, "zzz")
        self.assertIn("(no dependencies)", out)
