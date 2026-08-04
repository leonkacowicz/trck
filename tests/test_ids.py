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
        with mock.patch.object(t, "_existing_ids", return_value={"aaaaaaa"}), \
            mock.patch.object(t.secrets, "choice", side_effect=list("aaaaaaa") + list("bbbbbbb")):
            gid = t.gen_id(ctx)
        self.assertEqual(gid, "bbbbbbb")

    def test_an_all_digit_id_is_an_ordinary_id(self):
        """All-digit draws used to be redrawn, to keep `all-digit \u21d4 legacy integer
        id` sound. Nothing reads that discriminator now, so the alphabet is used in
        full and `2345678` is kept like any other draw."""
        t = self.t
        with mock.patch.object(t, "_existing_ids", return_value=set()), \
            mock.patch.object(t.secrets, "choice", side_effect=list("2345678") + list("abcdefg")):
            gid = t.gen_id(self._ctx())
        self.assertEqual(gid, "2345678")


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
        mk = lambda i: self.t.Issue(id=i, slug="s", title="T",
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

    def test_leading_hash_is_stripped_exact(self):
        # ids print as "#abc1234"; pasting that back must resolve
        self.assertEqual(self.t.resolve_ref(self.rows, "#p4abcde").id, "p4abcde")

    def test_leading_hash_is_stripped_prefix(self):
        self.assertEqual(self.t.resolve_ref(self.rows, "#p4").id, "p4abcde")

    def test_a_numeric_token_is_now_only_a_prefix(self):
        """The second resolution tier used to be `legacy_id`, so a numeric token meant
        "the issue that used to be #65" and beat a prefix match. Integer ids are gone,
        so digits are just characters in the alphabet and resolve as a prefix."""
        t = self.t
        rows = [t.Issue(id="65abcde", slug="s", title="T",
                        status="backlog", priority="high")]
        self.assertEqual(t.resolve_ref(rows, "65").id, "65abcde")
        with self.assertRaises(SystemExit):
            t.resolve_ref(rows, "99")

    def test_only_a_leading_hash_is_stripped(self):
        # a bare "#" is not an id; strip the one "#" and fall through to no-match
        with self.assertRaises(SystemExit):
            self.t.resolve_ref(self.rows, "#")


class TestMergeAndOrder(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_two_branch_rows_union_without_clash(self):
        # Two independently-generated ids never collide structurally: a dict keyed
        # by id keeps both rows with intact cross-references.
        t = self.t
        a = t.Issue(id="k3m9x2a", slug="a", title="A",
                    status="backlog", priority="high")
        b = t.Issue(id="p4abcde", slug="b", title="B",
                    status="backlog", priority="high", depends_on=["k3m9x2a"])
        by_id = {r.id: r for r in [a, b]}
        self.assertEqual(set(by_id), {"k3m9x2a", "p4abcde"})
        self.assertEqual(by_id["p4abcde"].depends_on, ["k3m9x2a"])

    def test_list_default_sort_is_created(self):
        # When --sort is unset, the parser leaves it None and cmd_list falls back
        # to "created" order.
        t = self.t
        self.assertIsNone(t.build_parser().parse_args(["list"]).sort)


class TestDepsRootResolution(unittest.TestCase):
    """`deps <id>` must resolve a prefix like every other id arg, not compare the raw
    token against resolved ids (regression: a prefix wrongly printed
    '(no dependencies)')."""
    def setUp(self):
        self.t = load_trck()

    def _tracker(self):
        t = self.t
        d = make_tracker(tempfile.mkdtemp())
        ctx = t.Ctx(d, t.load_config(d))
        a = t.Issue(id="aabbcc2", slug="a", title="Prereq",
                    status="backlog", priority="high", created=t.now_utc())
        b = t.Issue(id="ddee3f4", slug="b", title="Dependent",
                    status="backlog", priority="high", depends_on=["aabbcc2"],
                    created=t.now_utc())
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
        iso = t.Issue(id="zzz9k8m", slug="iso", title="Lonely",
                      status="backlog", priority="high", created=t.now_utc())
        p = t.issue_path(ctx, iso); p.parent.mkdir(parents=True, exist_ok=True)
        p.write_text(t.TEMPLATE.format(title="Lonely"))
        rows = t.load_index(ctx); rows.append(iso); t.save_index(ctx, rows)
        out = self._deps(ctx, "zzz")
        self.assertIn("(no dependencies)", out)


class TestPrefixHighlightVerbs(unittest.TestCase):
    """The shortest-unique-prefix highlight (from `list`) also applies to
    `ready`/`next`, `deps`, and `show`."""
    def setUp(self):
        self.t = load_trck()
        self.t._use_color = lambda: True   # force ANSI so the highlight is observable

    def _tracker(self):
        t = self.t
        d = make_tracker(tempfile.mkdtemp())
        ctx = t.Ctx(d, t.load_config(d))
        # two ids sharing the first char -> unique prefix length 2 each
        a = t.Issue(id="k3aaaab", slug="a", title="Alpha",
                    status="backlog", priority="high", created=t.now_utc())
        b = t.Issue(id="k9bbbbb", slug="b", title="Beta",
                    status="backlog", priority="high", created=t.now_utc())
        for r in (a, b):
            p = t.issue_path(ctx, r)
            p.parent.mkdir(parents=True, exist_ok=True)
            p.write_text(t.TEMPLATE.format(title=r.title))
        t.save_index(ctx, [a, b])
        return ctx

    def _capture(self, fn, args):
        buf = io.StringIO()
        with redirect_stdout(buf):
            fn(args)
        return buf.getvalue()

    def test_ready_highlights_unique_prefix(self):
        ctx = self._tracker()
        out = self._capture(self.t.cmd_ready, ns(dir=str(ctx.dir), next=False))
        self.assertIn(self.t.paint("k3", "bold"), out)     # prefix bold
        self.assertIn(self.t.paint("aaaab", "dim"), out)   # remainder dimmed
        self.assertNotIn(self.t.paint("#k3aaaab", "bold"), out)  # not whole-id bold

    def test_show_highlights_id_prefix(self):
        ctx = self._tracker()
        out = self._capture(self.t.cmd_show,
                            ns(dir=str(ctx.dir), id="k3aaaab", json=False))
        self.assertIn(self.t.paint("k3", "bold"), out)
        self.assertIn(self.t.paint("aaaab", "dim"), out)

    def test_show_json_is_unstyled(self):
        ctx = self._tracker()
        out = self._capture(self.t.cmd_show,
                            ns(dir=str(ctx.dir), id="k3aaaab", json=True))
        self.assertNotIn("\033[", out)                     # raw JSON carries no ANSI
        self.assertIn('"id": "k3aaaab"', out)
