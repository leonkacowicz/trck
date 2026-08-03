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

class TestFixedVocabulary(unittest.TestCase):
    """The four statuses are fixed: not configured, not renameable, not extensible.

    They were briefly two vocabularies — canonical states with per-tracker display names
    over them — which bought renaming and cost a second word for one concept. The names
    themselves are good; a tracker does not get to change them."""

    def setUp(self):
        self.t = load_trck()

    def test_the_vocabulary_is_the_four_names(self):
        self.assertEqual(self.t.STATUSES, ("backlog", "ongoing", "in-review", "done"))
        self.assertEqual(self.t.status_names(self.t.DEFAULT_CONFIG),
                         ["backlog", "ongoing", "in-review", "done"])

    def test_the_config_carries_no_vocabulary_at_all(self):
        """There is nothing to configure, so `trck.json` has no key for it. A tracker that
        never opens the file gets the right answer, which is the whole point."""
        self.assertNotIn("statuses", self.t.DEFAULT_CONFIG)
        self.assertNotIn("aliases", self.t.DEFAULT_CONFIG)

    def test_a_tracker_cannot_redefine_the_vocabulary(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {"statuses": [{"name": "wip"}, {"name": "shipped"}]})
            cfg = self.t.load_config(d)
            self.assertEqual(self.t.status_names(cfg),
                             ["backlog", "ongoing", "in-review", "done"])

    def test_a_leftover_vocabulary_key_warns_rather_than_breaks(self):
        """Every tracker written before this carries `statuses` and `aliases`. Ignoring
        them silently would hide a real surprise; erroring would lock the tracker out of
        every verb over a key that no longer does anything. So: a warning, naming what
        is fixed."""
        warns = self.t.check_vestigial_vocabulary({"statuses": [], "aliases": {}})
        self.assertEqual(len(warns), 2)
        self.assertTrue(all("no longer configurable" in w for w in warns), warns)
        self.assertEqual(self.t.check_vestigial_vocabulary(self.t.DEFAULT_CONFIG), [])

    def test_the_semantic_predicates_read_the_status_directly(self):
        cfg = self.t.DEFAULT_CONFIG
        self.assertEqual(self.t.initial_status(cfg), "backlog")
        self.assertEqual(self.t.active_status(cfg), "ongoing")
        self.assertEqual(self.t.terminal_statuses(cfg), ["done"])
        self.assertEqual([self.t.is_terminal(cfg, n) for n in self.t.STATUSES],
                         [False, False, False, True])

    def test_only_backlog_and_ongoing_offer_work_to_pick_up(self):
        """`in-review` is in flight but its own output is pending someone else\'s
        judgement, so there is nothing to start; `done` is finished. Actionability used to
        fail open for anything that had not opted out, so `done` answered True."""
        cfg = self.t.DEFAULT_CONFIG
        self.assertEqual([self.t.is_actionable(cfg, n) for n in self.t.STATUSES],
                         [True, True, False, False])

    def test_the_verb_aliases_are_constants(self):
        cfg = self.t.DEFAULT_CONFIG
        self.assertEqual([self.t.resolve_alias(cfg, v) for v in ("start", "review", "done")],
                         ["ongoing", "in-review", "done"])
        self.assertIsNone(self.t.resolve_alias(cfg, "nonesuch"))


class TestConfigRemainder(unittest.TestCase):
    def setUp(self):
        self.t = load_trck()

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
