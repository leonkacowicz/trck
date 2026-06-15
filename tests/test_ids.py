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
