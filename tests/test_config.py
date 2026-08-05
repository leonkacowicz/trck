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
            self.assertNotIn("priorities", cfg)
            self.assertNotIn("kinds", cfg)

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

    def test_check_priority_names_the_fixed_five(self):
        self.assertIsNone(self.t.check_priority(self.cfg, "high"))
        msg = self.t.check_priority(self.cfg, "bogus")
        self.assertIn("bad priority 'bogus'", msg)
        self.assertIn("urgent, high, medium, low, lowest", msg)  # names the fixed set

    def test_check_resolution_names_the_fixed_three(self):
        self.assertIsNone(self.t.check_resolution(self.cfg, "wontfix"))
        # 'fixed' is in the stale config above and still rejected: the config no longer
        # has a say.
        msg = self.t.check_resolution(self.cfg, "fixed")
        self.assertIn("bad resolution 'fixed'", msg)
        self.assertIn("superseded, wontfix, duplicate", msg)

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


class TestFixedPriorities(unittest.TestCase):
    """Five priorities, fixed — the same treatment the statuses got, for the same reason.

    The plan had said "names as display aliases", so a team could call them P0-P4. That
    criterion is struck: it is exactly what was deleted for statuses, and two words for
    one concept costs more than renaming buys. The names shipped are the names."""

    def setUp(self):
        self.t = load_trck()

    def test_the_five_are_constants(self):
        self.assertEqual(self.t.PRIORITIES,
                         ("urgent", "high", "medium", "low", "lowest"))

    def test_the_config_carries_no_priority_vocabulary(self):
        self.assertNotIn("priorities", self.t.DEFAULT_CONFIG)
        self.assertNotIn("default_priority", self.t.DEFAULT_CONFIG)

    def test_a_tracker_cannot_redefine_them(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {"priorities": ["p0", "p1"], "default_priority": "p0"})
            cfg = self.t.load_config(d)
            self.assertIsNone(self.t.check_priority(cfg, "urgent"))
            self.assertIsNotNone(self.t.check_priority(cfg, "p0"))
            self.assertEqual(self.t.default_priority(cfg), "medium")

    def test_the_leftover_keys_warn(self):
        warns = self.t.check_vestigial_vocabulary(
            {"priorities": [], "default_priority": "x"})
        self.assertEqual(len(warns), 2)
        self.assertTrue(all("no longer configurable" in w for w in warns), warns)

    def test_the_default_is_the_middle_one(self):
        self.assertEqual(self.t.default_priority(self.t.DEFAULT_CONFIG), "medium")
        self.assertEqual(self.t.PRIORITIES[len(self.t.PRIORITIES) // 2], "medium")

    def test_rank_orders_them_and_sinks_anything_else(self):
        rank = lambda p: self.t.priority_rank(self.t.DEFAULT_CONFIG, p)
        self.assertEqual([rank(p) for p in self.t.PRIORITIES], [0, 1, 2, 3, 4])
        # A hand-edited row can still carry junk; it sorts last rather than throwing.
        self.assertEqual(rank("nonesuch"), len(self.t.PRIORITIES))


class TestFixedResolutions(unittest.TestCase):
    """Three resolutions, fixed — the last vocabulary key, and the only one that was ever
    load-bearing rather than decorative.

    A resolution means *closed without shipping*. `select_shipped` skips any issue
    carrying one, so the field is what separates a changelog entry from a closed issue
    that produced nothing to announce."""

    def setUp(self):
        self.t = load_trck()

    def test_the_three_are_constants(self):
        self.assertEqual(self.t.RESOLUTIONS, ("superseded", "wontfix", "duplicate"))

    def test_the_config_is_now_empty_of_vocabulary_entirely(self):
        """Neither remaining key is a decision about how to track work: `format` is which
        shape the tracker is written in, `update` is where the engine comes from. Every
        key that *was* such a decision has been made once, for everyone."""
        self.assertEqual(set(self.t.DEFAULT_CONFIG), {"format", "update"})

    def test_a_tracker_cannot_redefine_them(self):
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {"resolutions": ["fixed", "invalid", "obsolete"]})
            cfg = self.t.load_config(d)
            self.assertIsNone(self.t.check_resolution(cfg, "duplicate"))
            self.assertIsNotNone(self.t.check_resolution(cfg, "obsolete"))

    def test_the_leftover_key_warns(self):
        warns = self.t.check_vestigial_vocabulary({"resolutions": ["fixed"]})
        self.assertEqual(len(warns), 1)
        self.assertIn("no longer configurable", warns[0])

    def test_there_is_deliberately_no_success_resolution(self):
        """`fixed`/`done`/`completed` would be the empty case spelled out — and setting
        one would silently drop the issue from the changelog it belongs in, since
        `select_shipped` keys off the field being *absent*."""
        for name in ("fixed", "done", "completed", "shipped", "resolved"):
            self.assertNotIn(name, self.t.RESOLUTIONS)


class TestFormatVersion(unittest.TestCase):
    """`trck.json` carries a format version, and an engine refuses a tracker written in
    a shape it does not understand.

    Until this existed, the vendored copy of the engine *was* the pin — the writer lived
    in the repo, so writer and reader could not disagree. Half the surface was already
    safe (`Issue.extra` round-trips unknown index keys), but config was not: a key an old
    engine has never heard of is silently ignored, and it then gives wrong answers with
    no way to notice."""

    def setUp(self):
        self.t = load_trck()

    def load(self, cfg, **kw):
        with TemporaryDirectory() as tmp:
            return self.t.load_config(make_tracker(tmp, cfg), **kw)

    def test_the_current_format_is_declared_in_constants(self):
        self.assertIsInstance(self.t.SUPPORTED_FORMAT, int)
        self.assertEqual(self.t.DEFAULT_CONFIG["format"], self.t.SUPPORTED_FORMAT)

    def test_an_absent_format_means_the_current_shape(self):
        """Every tracker written before this key existed. Treating absence as "ancient,
        refuse" would lock out every one of them."""
        self.assertIsNone(self.t.check_format({}))
        self.assertEqual(self.load({})["format"], self.t.SUPPORTED_FORMAT)

    def test_an_equal_format_is_accepted(self):
        self.assertIsNone(self.t.check_format({"format": self.t.SUPPORTED_FORMAT}))

    def test_an_older_format_is_accepted(self):
        """Refuse newer only. An old tracker is what the migration verbs are for, and
        refusing it would mean an engine could not run the migration that fixes it."""
        self.assertIsNone(self.t.check_format({"format": self.t.SUPPORTED_FORMAT - 1}))

    def test_a_newer_format_is_refused_and_names_the_fix(self):
        msg = self.t.check_format({"format": self.t.SUPPORTED_FORMAT + 1})
        self.assertIsNotNone(msg)
        self.assertIn(str(self.t.SUPPORTED_FORMAT + 1), msg)
        self.assertIn("upgrade trck", msg)

    def test_a_malformed_format_is_a_clean_error(self):
        for bad in ("1", 1.5, None, True, []):
            msg = self.t.check_format({"format": bad})
            self.assertIsNotNone(msg, bad)
            self.assertIn("must be an integer", msg)

    def test_every_verb_passes_through_the_guard(self):
        """One choke point: every verb builds a Ctx and every Ctx loads a config here,
        so the guard does not have to be remembered at each call site."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {"format": self.t.SUPPORTED_FORMAT + 1})
            err = io.StringIO()
            with redirect_stderr(err), self.assertRaises(SystemExit):
                self.t.load_config(d)
            self.assertIn("newer than this engine", err.getvalue())

    def test_update_is_exempt_from_the_guard(self):
        """`update` is the remedy the refusal tells you to run, so it must survive the
        thing it fixes. It reads only `update.repo`, a string in every format."""
        with TemporaryDirectory() as tmp:
            d = make_tracker(tmp, {"format": self.t.SUPPORTED_FORMAT + 99,
                                   "update": {"repo": "someone/trck"}})
            cfg = self.t.load_config(d, guard=False)
            self.assertEqual(cfg["update"]["repo"], "someone/trck")


class TestFormatExtensions(unittest.TestCase):
    """Extensions, not just an integer — git's model, for its granularity.

    A flat version pins the whole tracker, so bumping it for an opt-in feature would
    lock out old engines even for repos not using it. An extension key says "you may
    meet this; refuse it if you do not know it", so only the repos that opted in are
    affected."""

    def setUp(self):
        self.t = load_trck()
        self.known = self.t.KNOWN_EXTENSIONS

    def tearDown(self):
        self.t.KNOWN_EXTENSIONS = self.known

    def test_an_unknown_extension_is_refused_and_named(self):
        msg = self.t.check_format({"extensions": {"partial-clone": {}}})
        self.assertIsNotNone(msg)
        self.assertIn("partial-clone", msg)
        self.assertIn("upgrade trck", msg)

    def test_every_unknown_extension_is_named_not_just_the_first(self):
        msg = self.t.check_format({"extensions": {"zeta": {}, "alpha": {}}})
        self.assertIn("alpha", msg)
        self.assertIn("zeta", msg)

    def test_a_known_extension_is_accepted(self):
        self.t.KNOWN_EXTENSIONS = frozenset({"field-schema"})
        self.assertIsNone(self.t.check_format({"extensions": {"field-schema": {}}}))

    def test_no_extensions_is_the_common_case(self):
        self.assertIsNone(self.t.check_format({}))
        self.assertIsNone(self.t.check_format({"extensions": {}}))

    def test_a_malformed_extensions_block_is_a_clean_error(self):
        msg = self.t.check_format({"extensions": ["field-schema"]})
        self.assertIsNotNone(msg)
        self.assertIn("must be an object", msg)

    def test_an_extension_does_not_need_a_format_bump(self):
        """The whole point of the granularity: a tracker opting into an extension stays
        at the current format, so an engine that knows the extension runs it and one that
        does not refuses only that repo."""
        self.t.KNOWN_EXTENSIONS = frozenset({"field-schema"})
        self.assertIsNone(self.t.check_format(
            {"format": self.t.SUPPORTED_FORMAT, "extensions": {"field-schema": {}}}))
