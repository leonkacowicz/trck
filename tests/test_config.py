import io
import json
import os
import unittest
from contextlib import redirect_stderr
from pathlib import Path
from tempfile import TemporaryDirectory

from tests.helpers import load_trck, make_tracker


class TestConfigDefaults(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_empty_config_uses_defaults(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            cfg = self.t.load_config(d)
            self.assertEqual(self.t.status_names(cfg),
                             ["backlog", "ongoing", "in-review", "done"])
            self.assertEqual(self.t.initial_status(cfg), "backlog")
            self.assertTrue(self.t.is_terminal(cfg, "done"))
            self.assertFalse(self.t.is_terminal(cfg, "ongoing"))
            self.assertEqual(cfg["priorities"],
                             ["urgent", "high", "medium", "low", "lowest"])
            self.assertEqual(cfg["kinds"],
                             ["task", "epic", "bug", "story", "investigation"])

class TestSemanticStates(unittest.TestCase):
    """A status is a label over one of four states, not a free-form lifecycle position.

    The states were always there — `role` plus `actionable` encode exactly these four —
    but they were secondary, derived by each caller from two independent fields. Making
    the state primary is what lets the engine ask "is this in review?" instead of "does it
    lack a role and opt out of actionable?", and it is the vocabulary a Rust port and any
    cross-tracker tool can rely on."""

    def setUp(self):
        self.t = load_trck()

    def test_the_shipped_vocabulary_covers_every_state(self):
        cfg = self.t.DEFAULT_CONFIG
        self.assertEqual([self.t.state_of(cfg, n) for n in self.t.status_names(cfg)],
                         ["todo", "doing", "review", "done"])

    def test_a_state_may_be_declared_outright(self):
        cfg = {"statuses": [{"name": "icebox", "state": "todo"},
                            {"name": "qa", "state": "review"}]}
        self.assertEqual(self.t.state_of(cfg, "icebox"), "todo")
        self.assertEqual(self.t.state_of(cfg, "qa"), "review")

    def test_a_declared_state_wins_over_the_derived_one(self):
        """Derivation exists to read configs written before states did. Where a tracker
        says what it means, that is the answer — otherwise migrating a config would be
        unable to correct a mapping the old fields got wrong."""
        cfg = {"statuses": [{"name": "x", "role": "initial", "state": "doing"}]}
        self.assertEqual(self.t.state_of(cfg, "x"), "doing")

    def test_states_are_derived_from_the_old_role_and_flag(self):
        cfg = {"statuses": [{"name": "a", "role": "initial"},
                            {"name": "b", "role": "active"},
                            {"name": "c", "actionable": False},
                            {"name": "d", "role": "terminal"}]}
        self.assertEqual([self.t.state_of(cfg, n) for n in "abcd"],
                         ["todo", "doing", "review", "done"])

    def test_a_status_that_only_opts_out_of_actionable_is_in_review(self):
        # `actionable: false` says "in flight, nothing to pick up" whatever else it
        # carries — that is the whole content of `review`.
        cfg = {"statuses": [{"name": "blocked", "role": "active", "actionable": False}]}
        self.assertEqual(self.t.state_of(cfg, "blocked"), "review")

    def test_an_unknown_status_is_not_guessed_at(self):
        self.assertIsNone(self.t.state_of(self.t.DEFAULT_CONFIG, "nonesuch"))

    def test_predicates_read_the_state_not_the_old_fields(self):
        cfg = {"statuses": [{"name": "a", "state": "todo"}, {"name": "b", "state": "doing"},
                            {"name": "c", "state": "review"}, {"name": "d", "state": "done"}]}
        self.assertEqual(self.t.initial_status(cfg), "a")
        self.assertEqual(self.t.active_status(cfg), "b")
        self.assertEqual(self.t.terminal_statuses(cfg), ["d"])
        self.assertTrue(self.t.is_terminal(cfg, "d"))
        self.assertEqual([self.t.is_actionable(cfg, n) for n in "abcd"],
                         [True, True, False, False])

    def test_a_terminal_status_is_never_actionable(self):
        """`is_actionable` used to fail open for anything not opting out, so a terminal
        status answered True and readiness had to exclude it separately."""
        self.assertFalse(self.t.is_actionable(self.t.DEFAULT_CONFIG, "done"))

    def test_a_state_outside_the_four_is_rejected(self):
        cfg = {"statuses": [{"name": "x", "state": "pondering"}]}
        errs = self.t.check_status_states(cfg)
        self.assertTrue(any("pondering" in e for e in errs), errs)

    def test_the_rollup_anchors_must_each_be_named_once(self):
        """Rollup derives a parent's status from its children, so it needs exactly one
        status to mean todo, one doing and one done. `review` is exempt — a tracker may
        have several review states, and rollup never picks one."""
        two = {"statuses": [{"name": "a", "state": "todo"}, {"name": "b", "state": "todo"},
                            {"name": "c", "state": "doing"}, {"name": "d", "state": "done"}]}
        self.assertTrue(any("todo" in e for e in self.t.check_status_states(two)))
        many_reviews = {"statuses": [{"name": "a", "state": "todo"}, {"name": "b", "state": "doing"},
                                   {"name": "c", "state": "review"}, {"name": "d", "state": "review"},
                                   {"name": "e", "state": "done"}]}
        self.assertEqual(self.t.check_status_states(many_reviews), [])

    def test_active_status_from_role(self):
        # the shipped default marks `ongoing` as the active-role status
        self.assertEqual(self.t.active_status(self.t.DEFAULT_CONFIG), "ongoing")
        # honoured under a custom vocabulary
        cfg = {"statuses": [{"name": "todo", "role": "initial"},
                            {"name": "wip", "role": "active"},
                            {"name": "shipped", "role": "terminal"}]}
        self.assertEqual(self.t.active_status(cfg), "wip")
        # absent role -> None (no active status configured)
        self.assertIsNone(self.t.active_status({"statuses": [{"name": "a"}]}))

    def test_default_priority_explicit_invalid_and_fallback(self):
        dp = self.t.default_priority
        # explicit, valid -> wins
        self.assertEqual(dp({"priorities": ["a", "b", "c"],
                             "default_priority": "b"}), "b")
        # explicit, not in list -> median fallback
        self.assertEqual(dp({"priorities": ["a", "b", "c"],
                             "default_priority": "z"}), "b")
        # no key -> median of the configured list
        self.assertEqual(dp({"priorities": ["p0", "p1"]}), "p1")
        # shipped defaults resolve to medium
        self.assertEqual(dp(self.t.DEFAULT_CONFIG), "medium")

    def test_partial_config_overrides_only_given_keys(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {"priorities": ["p0", "p1"]})
            cfg = self.t.load_config(d)
            self.assertEqual(cfg["priorities"], ["p0", "p1"])
            self.assertEqual(self.t.status_names(cfg),
                             ["backlog", "ongoing", "in-review", "done"])

    def test_custom_statuses_and_roles(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {"statuses": [
                {"name": "todo", "role": "initial"},
                {"name": "doing"},
                {"name": "review"},
                {"name": "shipped", "role": "terminal"},
                {"name": "dropped", "role": "terminal"},
            ]})
            cfg = self.t.load_config(d)
            self.assertEqual(self.t.initial_status(cfg), "todo")
            self.assertEqual(set(self.t.terminal_statuses(cfg)), {"shipped", "dropped"})

    def test_initial_defaults_to_first_when_no_role(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {"statuses": [{"name": "a"}, {"name": "b"}]})
            cfg = self.t.load_config(d)
            self.assertEqual(self.t.initial_status(cfg), "a")

    def test_resolve_alias(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            cfg = self.t.load_config(d)
            self.assertEqual(self.t.resolve_alias(cfg, "start"), "ongoing")
            self.assertEqual(self.t.resolve_alias(cfg, "done"), "done")
            self.assertIsNone(self.t.resolve_alias(cfg, "nope"))


class TestVocabularyChecks(unittest.TestCase):
    """The shared predicate helpers: return None when valid, else a die-ready
    message that still lists the configured options."""

    def setUp(self):
        self.t = load_trck()
        self.cfg = {"priorities": ["high", "low"], "kinds": ["task", "bug"],
                    "resolutions": ["fixed", "wontfix"]}

    def test_check_priority(self):
        self.assertIsNone(self.t.check_priority(self.cfg, "high"))
        msg = self.t.check_priority(self.cfg, "bogus")
        self.assertIn("bad priority 'bogus'", msg)
        self.assertIn("high, low", msg)  # lists the configured set

    def test_check_kind(self):
        self.assertIsNone(self.t.check_kind(self.cfg, "bug"))
        msg = self.t.check_kind(self.cfg, "bogus")
        self.assertIn("bad kind 'bogus'", msg)
        self.assertIn("task, bug", msg)

    def test_check_resolution(self):
        self.assertIsNone(self.t.check_resolution(self.cfg, "fixed"))
        msg = self.t.check_resolution(self.cfg, "bogus")
        self.assertIn("bad resolution 'bogus'", msg)
        self.assertIn("fixed, wontfix", msg)

    def test_check_points(self):
        self.assertIsNone(self.t.check_points(0))
        self.assertIsNone(self.t.check_points(5))
        msg = self.t.check_points(-1)
        self.assertIn("bad points -1", msg)
        self.assertIn("non-negative integer", msg)


class TestDiscovery(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

    def test_from_inside_tracker(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.assertEqual(self.t.find_tracker(d), d.resolve())

    def test_from_sibling_walks_up(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            deep = Path(tmp) / "src" / "x"
            deep.mkdir(parents=True)
            self.assertEqual(self.t.find_tracker(deep), d.resolve())

    def test_none_found_raises(self):
        with TemporaryDirectory() as tmp, self.assertRaises(SystemExit):
            self.t.find_tracker(Path(tmp))

    def test_optional_resolution_returns_none(self):
        with TemporaryDirectory() as tmp:
            # not-found, but required=False -> None instead of die (no stderr)
            self.assertIsNone(self.t.find_tracker(Path(tmp), required=False))
            self.assertIsNone(
                self.t.resolve_tracker_dir(str(tmp), env={}, required=False)
            )
            # TRCK_DIR pointing at a non-tracker, required=False -> None (recursion path)
            self.assertIsNone(
                self.t.resolve_tracker_dir(None, env={"TRCK_DIR": str(tmp)}, required=False)
            )

    def test_ambiguous_raises(self):
        with TemporaryDirectory() as tmp:
            make_tracker(tmp, {})
            (Path(tmp) / "other").mkdir()
            (Path(tmp) / "other" / "trck.json").write_text("{}")
            with self.assertRaises(SystemExit):
                self.t.find_tracker(Path(tmp))

    def test_env_override(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.assertEqual(
                self.t.resolve_tracker_dir(None, env={"TRCK_DIR": str(d)}), d.resolve()
            )

    def test_dir_arg_wins(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {})
            self.assertEqual(self.t.resolve_tracker_dir(str(d), env={}), d.resolve())

    def test_or_die_no_tracker_no_dir_is_clean_error(self):
        # dir_opt is None and nothing resolves: a clean `error: …` message,
        # not a Python traceback (regression for the Path(None) TypeError).
        with TemporaryDirectory() as tmp:
            self.t.SELF_PATH = Path(tmp) / "trck"  # parent has no trck.json
            cwd = os.getcwd()
            err = io.StringIO()
            try:
                os.chdir(tmp)
                with redirect_stderr(err), self.assertRaises(SystemExit):
                    self.t.resolve_tracker_dir_or_die(None, env={})
            finally:
                os.chdir(cwd)
        self.assertIn("no tracker found here", err.getvalue())

    def test_or_die_explicit_invalid_dir_names_path(self):
        # An explicit but invalid --dir still produces a message naming the path.
        with TemporaryDirectory() as tmp:
            err = io.StringIO()
            with redirect_stderr(err), self.assertRaises(SystemExit):
                self.t.resolve_tracker_dir_or_die(str(tmp), env={})
            out = err.getvalue()
            self.assertIn("is not a tracker", out)
            self.assertIn(str(Path(tmp).resolve()), out)

    def test_or_die_explicit_invalid_env_names_path(self):
        # Same for an explicit but invalid $TRCK_DIR (no --dir).
        with TemporaryDirectory() as tmp:
            err = io.StringIO()
            with redirect_stderr(err), self.assertRaises(SystemExit):
                self.t.resolve_tracker_dir_or_die(None, env={"TRCK_DIR": str(tmp)})
            out = err.getvalue()
            self.assertIn("is not a tracker", out)
            self.assertIn(str(Path(tmp).resolve()), out)
