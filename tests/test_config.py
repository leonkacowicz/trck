import json
import unittest
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
            self.assertEqual(self.t.status_names(cfg), ["backlog", "ongoing", "done"])
            self.assertEqual(self.t.initial_status(cfg), "backlog")
            self.assertTrue(self.t.is_terminal(cfg, "done"))
            self.assertFalse(self.t.is_terminal(cfg, "ongoing"))
            self.assertEqual(cfg["priorities"],
                             ["urgent", "high", "medium", "low", "lowest"])
            self.assertEqual(cfg["kinds"],
                             ["task", "epic", "bug", "story", "investigation"])

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
            self.assertEqual(self.t.status_names(cfg), ["backlog", "ongoing", "done"])

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
        with TemporaryDirectory() as tmp:
            with self.assertRaises(SystemExit):
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
