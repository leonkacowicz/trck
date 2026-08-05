"""The `in-review` state, the built-in `review_url` field, and the `review` alias verb."""
import io
import json
import unittest
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker, ns


class Base(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    # -- helpers -------------------------------------------------------------
    def seed(self, d, title="Item", **over):
        args = ns(dir=str(d), title=title, priority=over.pop("priority", "high"),
                  parent=over.pop("parent", None),
                  depends=over.pop("depends", None), spec=None, slug=None,
                  points=over.pop("points", None), review_url=over.pop("review_url", None))
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(args)
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def mv(self, d, iid, status, **over):
        args = ns(dir=str(d), id=iid, status=status,
                  resolution=over.pop("resolution", None), review_url=over.pop("review_url", None))
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_mv(args)

    def set_(self, d, iid, **over):
        args = ns(dir=str(d), id=iid, priority=None, points=None, parent=None,
                  spec=None, title=None, slug=None, field=None,
                  unset=None, review_url=over.pop("review_url", None))
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_set(args)

    def review(self, d, iid, url=None):
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_review(ns(dir=str(d), id=iid, url=url))
        return buf.getvalue()

    def ctx(self, d):
        return self.t.Ctx(d, self.t.load_config(d))

    def rows(self, d):
        return {r.id: r for r in self.t.load_index(self.ctx(d))}

    def errors(self, d):
        return self.t.validate(self.ctx(d))[0]


# --------------------------------------------------------------------------- #
# vocabulary
# --------------------------------------------------------------------------- #
class TestInReviewVocabulary(Base):
    def test_default_vocabulary_includes_in_review(self):
        cfg = self.t.DEFAULT_CONFIG
        self.assertEqual(self.t.status_names(cfg),
                         ["backlog", "ongoing", "in-review", "done"])
        # in flight, but its own output is pending someone else's judgement, so there
        # is nothing here to pick up — and it is not finished either
        self.assertFalse(self.t.is_terminal(cfg, "in-review"))
        self.assertFalse(self.t.is_actionable(cfg, "in-review"))
        self.assertEqual(self.t.initial_status(cfg), "backlog")
        self.assertEqual(self.t.active_status(cfg), "ongoing")
        self.assertEqual(self.t.terminal_statuses(cfg), ["done"])

    def test_review_alias_is_configured_by_default(self):
        self.assertEqual(self.t.resolve_alias(self.t.DEFAULT_CONFIG, "review"),
                         "in-review")

    def test_only_todo_and_doing_offer_work_to_pick_up(self):
        """Actionability reads the state now, rather than failing open for anything that
        did not opt out. `done` therefore answers False where it used to answer True —
        readiness always excluded it separately, so nothing downstream changes, but the
        predicate no longer claims a finished issue is something to start."""
        cfg = self.t.DEFAULT_CONFIG
        self.assertEqual([self.t.is_actionable(cfg, n) for n in self.t.status_names(cfg)],
                         [True, True, False, False])
        # a status the vocabulary doesn't describe still fails open
        self.assertTrue(self.t.is_actionable(cfg, "nonesuch"))
        self.assertTrue(self.t.is_actionable(
            {"statuses": [{"name": "qa", "actionable": True}]}, "qa"))

    def test_a_leftover_statuses_key_is_a_warning_not_an_error(self):
        """Every tracker written before the vocabulary was fixed carries this key. It is
        ignored, so the tracker is not broken — erroring would lock it out of every verb
        over something that no longer does anything."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {"statuses": [{"name": "qa"}]})
            ctx = self.ctx(d)
            errors, warnings = self.t.validate(ctx)
            self.assertEqual(errors, [])
            self.assertTrue(any("no longer configurable" in w for w in warnings), warnings)


# --------------------------------------------------------------------------- #
# ready / next
# --------------------------------------------------------------------------- #
class TestActionableGatesReady(Base):
    def ready_ids(self, d):
        ctx = self.ctx(d)
        g = self.t.Graph(ctx.cfg, self.t.load_index(ctx))
        return {r.id for r in g.rows if g.is_ready(r)}

    def test_in_review_leaf_is_not_ready(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A")
            self.assertIn(a, self.ready_ids(d))
            self.mv(d, a, "in-review")
            self.assertNotIn(a, self.ready_ids(d))

    def test_moving_back_to_ongoing_makes_it_ready_again(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A")
            self.mv(d, a, "in-review")
            self.mv(d, a, "ongoing")
            self.assertIn(a, self.ready_ids(d))

    def test_next_skips_an_in_review_issue(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            hot = self.seed(d, "Hot", priority="urgent")
            cool = self.seed(d, "Cool", priority="low")
            self.mv(d, hot, "in-review")
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_next(ns(dir=str(d), id=None))
            out = buf.getvalue()
            self.assertIn(cool, out)
            self.assertNotIn(hot, out)

    def test_in_review_still_blocks_its_dependents(self):
        # non-terminal means not merged: work waiting on the PR stays blocked
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            base = self.seed(d, "Base")
            after = self.seed(d, "After", depends=base)
            self.mv(d, base, "in-review")
            ctx = self.ctx(d)
            g = self.t.Graph(ctx.cfg, self.t.load_index(ctx))
            self.assertTrue(g.is_blocked(g.by_id[after]))
            self.assertNotIn(after, self.ready_ids(d))

    def test_parent_of_an_in_review_child_rolls_up_to_active(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            epic = self.seed(d, "Epic")
            kid = self.seed(d, "Kid", parent=epic)
            self.mv(d, kid, "in-review")
            self.assertEqual(self.rows(d)[epic].status, "ongoing")
            self.assertEqual(self.errors(d), [])

# --------------------------------------------------------------------------- #
# the review_url field
# --------------------------------------------------------------------------- #
URL = "https://github.com/leonkacowicz/trck/pull/12"


class TestReviewUrlField(Base):
    def raw(self, d):
        return [json.loads(l) for l in
                (Path(d) / "index.jsonl").read_text().splitlines() if l.strip()]

    def test_review_url_is_a_canonical_field_after_spec(self):
        keys = self.t.CANON_KEYS
        self.assertEqual(keys[keys.index("spec") + 1], "review_url")
        self.assertIsNone(self.t.Issue(id="a", slug="s", title="T",
                                       status="backlog", priority="low").review_url)

    def test_absent_review_url_is_not_serialized(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            self.assertNotIn("review_url", self.raw(d)[0])

    def test_review_url_round_trips_through_the_index(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A", review_url=URL)
            self.assertEqual(self.raw(d)[0]["review_url"], URL)
            self.assertEqual(self.rows(d)[a].review_url, URL)

    def test_check_review_url_accepts_http_urls_only(self):
        self.assertIsNone(self.t.check_review_url(URL))
        self.assertIsNone(self.t.check_review_url("http://example.test/pr/1"))
        for bad in ("", "not a url", "example.com/pr/1", "ftp://x/y",
                    "https://has space/x"):
            self.assertIsNotNone(self.t.check_review_url(bad), bad)
        self.assertIn("http", self.t.check_review_url("nope"))

    def test_from_dict_rejects_a_non_string_review_url(self):
        with self.assertRaises(ValueError):
            self.t.Issue.from_dict({"id": "a", "slug": "s", "title": "T",
                                    "kind": "task", "status": "backlog",
                                    "priority": "low", "review_url": 12})

    def test_check_reports_a_malformed_review_url(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A")
            path = Path(d) / "index.jsonl"
            row = json.loads(path.read_text().strip())
            row["review_url"] = "pull/12"
            path.write_text(json.dumps(row) + "\n")
            self.assertTrue(any("review_url" in e for e in self.errors(d)))

    def test_review_url_is_reserved_against_custom_fields(self):
        msg = self.t.check_field_key("review_url")
        self.assertIsNotNone(msg)
        self.assertIn("built-in", msg)

class TestReviewUrlCli(Base):
    def test_every_entry_point_rejects_a_bad_url(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            err = io.StringIO()
            with redirect_stderr(err), self.assertRaises(SystemExit):
                self.seed(d, "A", review_url="nope")
            a = self.seed(d, "A")
            with redirect_stderr(err), self.assertRaises(SystemExit):
                self.set_(d, a, review_url="nope")
            with redirect_stderr(err), self.assertRaises(SystemExit):
                self.mv(d, a, "ongoing", review_url="nope")
            self.assertEqual(self.rows(d)[a].review_url, None)


# --------------------------------------------------------------------------- #
# the review verb
# --------------------------------------------------------------------------- #
class TestReviewVerb(Base):
    def test_review_rejects_a_bad_url_before_moving(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A")
            err = io.StringIO()
            with redirect_stderr(err), self.assertRaises(SystemExit):
                self.review(d, a, "pull/12")
            self.assertEqual(self.rows(d)[a].status, "backlog")

# --------------------------------------------------------------------------- #
# rendering
# --------------------------------------------------------------------------- #
class TestReviewUrlRendering(Base):
    def summary(self, d):
        return self.t.generate_summary(self.ctx(d))

    def test_review_url_tag_is_empty_without_a_pr(self):
        r = self.t.Issue(id="a", slug="s", title="T",
                         status="backlog", priority="low")
        self.assertEqual(self.t.review_tag(r), "")
        r.review_url = URL
        self.assertIn(URL, self.t.review_tag(r))

    def test_summary_links_a_standalone_issues_review_url(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A", review_url=URL)
            self.assertIn(f"[review]({URL})", self.summary(d))

    def test_summary_links_a_parent_and_its_child(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            epic = self.seed(d, "Epic")
            kid = self.seed(d, "Kid", parent=epic, review_url=URL)
            self.set_(d, epic, review_url="https://example.test/pull/1")
            text = self.summary(d)
            self.assertIn("Review: [https://example.test/pull/1]", text)
            self.assertIn(f"[review]({URL})", text)

    def test_summary_without_review_urls_is_unchanged(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            self.assertNotIn("PR", self.summary(d))

    def test_show_field_reads_canonical_fields_too(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A", review_url=URL)
            b = self.seed(d, "B")
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_list(ns(dir=str(d), id=None, flat=True, all=True,
                                   status=None, priority=None,
                                   label=None, parent=None, match=None, field=None,
                                   show_field=["review_url"], sort=None, blocked=False,
                                   orphan=False, paths=False))
            out = buf.getvalue()
            self.assertIn(f"review_url={URL}", out)
            # the row without one carries no empty column
            self.assertNotIn("review_url=\n", out)


if __name__ == "__main__":
    unittest.main()


class TestPrMigratesToReviewUrl(Base):
    """`pr` was named for the common case; what the field records is wherever the
    in-review output is being judged — a PR, a design doc, a vendor ticket. The rename
    is read-time, so an existing tracker keeps working and is rewritten on its next
    mutation."""

    def raw(self, d):
        return [json.loads(l) for l in
                (Path(d) / "index.jsonl").read_text().splitlines() if l.strip()]

    BASE = {"id": "a1b2c3d", "slug": "s", "title": "T",
            "status": "backlog", "priority": "low"}

    def test_a_legacy_pr_row_loads_as_review_url(self):
        r = self.t.Issue.from_dict({**self.BASE, "pr": URL})
        self.assertEqual(r.review_url, URL)
        self.assertNotIn("pr", r.extra)      # not left behind as a custom field
        self.assertNotIn("pr", r.to_canonical())

    def test_an_explicit_review_url_wins_over_a_stale_pr(self):
        r = self.t.Issue.from_dict(
            {**self.BASE, "pr": "https://example.test/old", "review_url": URL})
        self.assertEqual(r.review_url, URL)

    def test_a_legacy_row_is_rewritten_on_the_next_mutation(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A")
            path = Path(d) / "index.jsonl"
            row = json.loads(path.read_text().strip())
            row["pr"] = URL
            path.write_text(json.dumps(row) + "\n")
            self.assertEqual(self.rows(d)[a].review_url, URL)
            self.set_(d, a, points=2)
            self.assertEqual(self.raw(d)[0]["review_url"], URL)
            self.assertNotIn("pr", self.raw(d)[0])

    def test_the_legacy_names_are_not_available_as_custom_fields(self):
        """Both would be swallowed by a migration on the next load, so `--field pr=…`
        has to be refused rather than silently rewritten."""
        for key, hint in (("pr", "review_url"), ("milestone", "label")):
            msg = self.t.check_field_key(key)
            self.assertIsNotNone(msg, key)
            self.assertIn("legacy", msg)
            self.assertIn(hint, msg)
