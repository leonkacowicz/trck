"""`trck new --id ID` — supply an id instead of generating one.

Two customers, which is why it is a feature rather than a test hook: conformance
fixtures need a `new` whose result they can name in a golden, and importing issues from
another tracker (or restoring one deleted by hand) needs their ids preserved.

The rejected alternatives are recorded in #jpash72. The short version: normalising
generated ids in the runner is lossy — two ids emitted swapped would normalise to match
and pass — and seeding the generator would make id *generation* part of the conformance
contract, which Rust cannot reproduce from CPython portably."""
import io
import json
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class TestExplicitId(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def new(self, d, title="Item", **over):
        args = ns(dir=str(d), title=title, priority=over.pop("priority", "high"),
                  points=over.pop("points", None), parent=over.pop("parent", None),
                  depends=over.pop("depends", None), spec=None, slug=over.pop("slug", None),
                  review_url=None, id=over.pop("id", None))
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(args)
        return buf.getvalue().strip()

    def rows(self, d):
        ctx = self.t.Ctx(Path(d), self.t.load_config(Path(d)))
        return self.t.load_index(ctx)

    def test_the_supplied_id_is_used_verbatim(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            path = self.new(d, "Alpha", id="aaaaaaa")
            self.assertEqual([r.id for r in self.rows(d)], ["aaaaaaa"])
            self.assertTrue(path.endswith("aaaaaaa-alpha.md"))
            self.assertTrue(Path(path).is_file())

    def test_without_the_flag_nothing_changes(self):
        """The generated path keeps the same `gen_id` and the same collision guard —
        the flag is an alternative source for the value, not a second code path."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "Alpha")
            iid = self.rows(d)[0].id
            self.assertEqual(len(iid), self.t.ID_LEN)
            self.assertTrue(set(iid) <= set(self.t.ID_ALPHABET))

    def test_it_refuses_an_id_already_in_the_index(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "Alpha", id="aaaaaaa")
            err = io.StringIO()
            with redirect_stderr(err), self.assertRaises(SystemExit):
                self.new(d, "Beta", id="aaaaaaa")
            self.assertIn("aaaaaaa", err.getvalue())
            self.assertEqual(len(self.rows(d)), 1)

    def test_it_refuses_an_id_only_present_on_disk(self):
        """`gen_id` guards against index ∪ filesystem, because a branch may carry a body
        file whose index line has not merged yet. Supplying an id has to clear the same
        bar, or `--id` becomes the one way to reintroduce the collision random ids were
        adopted to prevent."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            items = Path(d) / "items"
            items.mkdir(exist_ok=True)
            (items / "bbbbbbb-ghost.md").write_text("# Ghost\n")
            err = io.StringIO()
            with redirect_stderr(err), self.assertRaises(SystemExit):
                self.new(d, "Beta", id="bbbbbbb")
            self.assertIn("bbbbbbb", err.getvalue())

    def test_it_refuses_an_id_outside_the_alphabet_or_length(self):
        """Without this the flag is a way to corrupt a tracker by hand: `0`/`1`/`o`/`l`/`i`
        are excluded from the alphabet precisely because they are misread, and a
        wrong-length id breaks the prefix-resolution the whole CLI leans on."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            for bad in ("short", "waytoolongid", "aaaaaa0", "AAAAAAA", "aaa-aaa", ""):
                err = io.StringIO()
                with redirect_stderr(err), self.assertRaises(SystemExit, msg=bad):
                    self.new(d, "X", id=bad)
                self.assertIn("bad id", err.getvalue(), bad)
            self.assertEqual(self.rows(d), [])

    def test_an_all_digit_id_is_accepted(self):
        """Digits are in the alphabet. They used to be reserved for integer ids; that
        namespace is gone (#dfe48ds), so `2345678` is an ordinary id."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.new(d, "Alpha", id="2345678")
            self.assertEqual([r.id for r in self.rows(d)], ["2345678"])

    def test_supplied_ids_make_a_whole_tracker_reproducible(self):
        """What the conformance fixtures actually need: the same setup twice, byte for
        byte, including the links between issues."""
        def build(d):
            self.new(d, "Prereq", id="aaaaaaa")
            self.new(d, "Epic", id="bbbbbbb")
            self.new(d, "Child", id="ccccccc", parent="bbbbbbb", depends="aaaaaaa")
            return (Path(d) / "index.jsonl").read_text()

        with TemporaryDirectory() as t1, TemporaryDirectory() as t2:
            a = build(make_tracker(t1, {}))
            b = build(make_tracker(t2, {}))
            # `created` is a real clock, so compare everything else.
            strip = lambda s: [{k: v for k, v in json.loads(l).items() if k != "created"}
                               for l in s.splitlines() if l.strip()]
            self.assertEqual(strip(a), strip(b))
            self.assertEqual([r["id"] for r in strip(a)],
                             ["aaaaaaa", "bbbbbbb", "ccccccc"])

    def test_the_flag_is_wired_on_new_only(self):
        """Changing an existing issue's id would have to rewrite every parent/depends_on
        pointing at it and rename its body file. That is a different feature."""
        p = self.t.build_parser()
        self.assertEqual(p.parse_args(["new", "T", "--id", "aaaaaaa"]).id, "aaaaaaa")
        with self.assertRaises(SystemExit):
            p.parse_args(["set", "aaaaaaa", "--id", "bbbbbbb"])
