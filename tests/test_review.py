"""The `in-review` waiting state, the `actionable` status flag, the built-in `pr`
field, and the `review` alias verb."""
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
                  kind=over.pop("kind", None), parent=over.pop("parent", None),
                  depends=over.pop("depends", None), spec=None, slug=None,
                  points=over.pop("points", None), pr=over.pop("pr", None))
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_new(args)
        return Path(buf.getvalue().strip()).name.split("-")[0]

    def mv(self, d, iid, status, **over):
        args = ns(dir=str(d), id=iid, status=status,
                  resolution=over.pop("resolution", None), pr=over.pop("pr", None))
        buf = io.StringIO()
        with redirect_stdout(buf):
            self.t.cmd_mv(args)

    def set_(self, d, iid, **over):
        args = ns(dir=str(d), id=iid, priority=None, points=None, parent=None,
                  spec=None, kind=None, title=None, slug=None, field=None,
                  unset=None, pr=over.pop("pr", None))
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
        # it is a waiting state, not a lifecycle anchor: no role at all
        self.assertIsNone(self.t.status_role(cfg, "in-review"))
        self.assertFalse(self.t.is_terminal(cfg, "in-review"))
        # the one-each role constraint still holds
        self.assertEqual(self.t.check_status_roles(cfg), [])
        self.assertEqual(self.t.initial_status(cfg), "backlog")
        self.assertEqual(self.t.active_status(cfg), "ongoing")
        self.assertEqual(self.t.terminal_statuses(cfg), ["done"])

    def test_review_alias_is_configured_by_default(self):
        self.assertEqual(self.t.resolve_alias(self.t.DEFAULT_CONFIG, "review"),
                         "in-review")

    def test_is_actionable_defaults_true_and_honours_opt_out(self):
        cfg = self.t.DEFAULT_CONFIG
        for name in ("backlog", "ongoing", "done"):
            self.assertTrue(self.t.is_actionable(cfg, name), name)
        self.assertFalse(self.t.is_actionable(cfg, "in-review"))
        # a status the vocabulary doesn't know is actionable (fail-open)
        self.assertTrue(self.t.is_actionable(cfg, "nonesuch"))
        # an explicit true is honoured
        self.assertTrue(self.t.is_actionable(
            {"statuses": [{"name": "qa", "actionable": True}]}, "qa"))

    def test_check_status_flags_rejects_non_boolean_actionable(self):
        self.assertEqual(self.t.check_status_flags(self.t.DEFAULT_CONFIG), [])
        msgs = self.t.check_status_flags(
            {"statuses": [{"name": "qa", "actionable": "no"}]})
        self.assertEqual(len(msgs), 1)
        self.assertIn("qa", msgs[0])
        self.assertIn("actionable", msgs[0])

    def test_check_reports_a_non_boolean_actionable(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {"statuses": [
                {"name": "backlog", "role": "initial"},
                {"name": "ongoing", "role": "active"},
                {"name": "qa", "actionable": "yes"},
                {"name": "done", "role": "terminal"},
            ]})
            self.assertTrue(any("actionable" in e for e in self.errors(d)))


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
            epic = self.seed(d, "Epic", kind="epic")
            kid = self.seed(d, "Kid", parent=epic)
            self.mv(d, kid, "in-review")
            self.assertEqual(self.rows(d)[epic].status, "ongoing")
            self.assertEqual(self.errors(d), [])

    def test_custom_vocabulary_can_opt_any_status_out(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {"statuses": [
                {"name": "todo", "role": "initial"},
                {"name": "wip", "role": "active"},
                {"name": "qa", "actionable": False},
                {"name": "shipped", "role": "terminal"},
            ]})
            a = self.seed(d, "A")
            self.mv(d, a, "qa")
            self.assertNotIn(a, self.ready_ids(d))


# --------------------------------------------------------------------------- #
# the pr field
# --------------------------------------------------------------------------- #
URL = "https://github.com/leonkacowicz/trck/pull/12"


class TestPrField(Base):
    def raw(self, d):
        return [json.loads(l) for l in
                (Path(d) / "index.jsonl").read_text().splitlines() if l.strip()]

    def test_pr_is_a_canonical_field_after_spec(self):
        keys = self.t.CANON_KEYS
        self.assertEqual(keys[keys.index("spec") + 1], "pr")
        self.assertIsNone(self.t.Issue(id="a", slug="s", title="T", kind="task",
                                       status="backlog", priority="low").pr)

    def test_absent_pr_is_not_serialized(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            self.assertNotIn("pr", self.raw(d)[0])

    def test_pr_round_trips_through_the_index(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A", pr=URL)
            self.assertEqual(self.raw(d)[0]["pr"], URL)
            self.assertEqual(self.rows(d)[a].pr, URL)

    def test_check_pr_accepts_http_urls_only(self):
        self.assertIsNone(self.t.check_pr(URL))
        self.assertIsNone(self.t.check_pr("http://example.test/pr/1"))
        for bad in ("", "not a url", "example.com/pr/1", "ftp://x/y",
                    "https://has space/x"):
            self.assertIsNotNone(self.t.check_pr(bad), bad)
        self.assertIn("http", self.t.check_pr("nope"))

    def test_from_dict_rejects_a_non_string_pr(self):
        with self.assertRaises(ValueError):
            self.t.Issue.from_dict({"id": "a", "slug": "s", "title": "T",
                                    "kind": "task", "status": "backlog",
                                    "priority": "low", "pr": 12})

    def test_check_reports_a_malformed_pr(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A")
            path = Path(d) / "index.jsonl"
            row = json.loads(path.read_text().strip())
            row["pr"] = "pull/12"
            path.write_text(json.dumps(row) + "\n")
            self.assertTrue(any("pr" in e for e in self.errors(d)))

    def test_pr_is_reserved_against_custom_fields(self):
        msg = self.t.check_field_key("pr")
        self.assertIsNotNone(msg)
        self.assertIn("built-in", msg)

    def test_show_prints_the_pr(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A", pr=URL)
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_show(ns(dir=str(d), id=a, json=False))
            self.assertIn(URL, buf.getvalue())


class TestPrCli(Base):
    def test_new_pr_stores_it(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A", pr=URL)
            self.assertEqual(self.rows(d)[a].pr, URL)
            self.assertEqual(self.errors(d), [])

    def test_set_pr_sets_and_clears(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A")
            self.set_(d, a, pr=URL)
            self.assertEqual(self.rows(d)[a].pr, URL)
            self.set_(d, a, pr="none")
            self.assertIsNone(self.rows(d)[a].pr)

    def test_mv_records_the_pr_as_part_of_the_move(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A")
            self.mv(d, a, "in-review", pr=URL)
            row = self.rows(d)[a]
            self.assertEqual(row.status, "in-review")
            self.assertEqual(row.pr, URL)
            self.assertEqual(self.errors(d), [])

    def test_every_entry_point_rejects_a_bad_url(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            err = io.StringIO()
            with redirect_stderr(err), self.assertRaises(SystemExit):
                self.seed(d, "A", pr="nope")
            a = self.seed(d, "A")
            with redirect_stderr(err), self.assertRaises(SystemExit):
                self.set_(d, a, pr="nope")
            with redirect_stderr(err), self.assertRaises(SystemExit):
                self.mv(d, a, "ongoing", pr="nope")
            self.assertEqual(self.rows(d)[a].pr, None)


# --------------------------------------------------------------------------- #
# the review verb
# --------------------------------------------------------------------------- #
class TestReviewVerb(Base):
    def test_review_moves_to_the_aliased_status(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A")
            self.review(d, a)
            self.assertEqual(self.rows(d)[a].status, "in-review")
            self.assertIsNone(self.rows(d)[a].pr)

    def test_review_with_a_url_moves_and_links_in_one_step(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A")
            out = self.review(d, a, URL)
            row = self.rows(d)[a]
            self.assertEqual(row.status, "in-review")
            self.assertEqual(row.pr, URL)
            self.assertEqual(len(out.strip().splitlines()), 1)  # one move, one line
            self.assertEqual(self.errors(d), [])

    def test_review_rejects_a_bad_url_before_moving(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A")
            err = io.StringIO()
            with redirect_stderr(err), self.assertRaises(SystemExit):
                self.review(d, a, "pull/12")
            self.assertEqual(self.rows(d)[a].status, "backlog")

    def test_review_without_the_alias_configured_points_at_mv(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {"aliases": {"start": "ongoing"}})
            a = self.seed(d, "A")
            err = io.StringIO()
            with redirect_stderr(err), self.assertRaises(SystemExit):
                self.review(d, a)
            self.assertIn("no 'review' alias configured", err.getvalue())
            self.assertIn("trck mv", err.getvalue())


# --------------------------------------------------------------------------- #
# rendering
# --------------------------------------------------------------------------- #
class TestPrRendering(Base):
    def summary(self, d):
        return self.t.generate_summary(self.ctx(d))

    def test_pr_tag_is_empty_without_a_pr(self):
        r = self.t.Issue(id="a", slug="s", title="T", kind="task",
                         status="backlog", priority="low")
        self.assertEqual(self.t.pr_tag(r), "")
        r.pr = URL
        self.assertIn(URL, self.t.pr_tag(r))

    def test_summary_links_a_standalone_issues_pr(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A", pr=URL)
            self.assertIn(f"[PR]({URL})", self.summary(d))

    def test_summary_links_a_parent_and_its_child(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            epic = self.seed(d, "Epic", kind="epic")
            kid = self.seed(d, "Kid", parent=epic, pr=URL)
            self.set_(d, epic, pr="https://example.test/pull/1")
            text = self.summary(d)
            self.assertIn("PR: [https://example.test/pull/1]", text)
            self.assertIn(f"[PR]({URL})", text)

    def test_summary_without_prs_is_unchanged(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.seed(d, "A")
            self.assertNotIn("PR", self.summary(d))

    def test_show_field_reads_canonical_fields_too(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            a = self.seed(d, "A", pr=URL)
            b = self.seed(d, "B")
            buf = io.StringIO()
            with redirect_stdout(buf):
                self.t.cmd_list(ns(dir=str(d), id=None, flat=True, all=True,
                                   status=None, kind=None, priority=None,
                                   label=None, parent=None, match=None, field=None,
                                   show_field=["pr"], sort=None, blocked=False,
                                   orphan=False, paths=False))
            out = buf.getvalue()
            self.assertIn(f"pr={URL}", out)
            # the row without one carries no empty column
            self.assertNotIn("pr=\n", out)


if __name__ == "__main__":
    unittest.main()
