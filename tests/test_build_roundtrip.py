"""Whole-engine build invariants.

Unlike `test_build.py` (synthetic fixtures), these run against the real
`src/trck/` package and the generated `./trck`. They guard that the amalgamation
stays faithful: valid Python, a single header, no leaked per-module imports, and
byte-identical to the engine on disk. (`tests/__init__.py` regenerates `./trck`
from `src/` before the suite, so the equality check reflects the current source.)
"""
import ast
import importlib.machinery
import importlib.util
import unittest
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent


def load_build():
    loader = importlib.machinery.SourceFileLoader("trck_build", str(REPO_ROOT / "build.py"))
    spec = importlib.util.spec_from_file_location("trck_build", REPO_ROOT / "build.py", loader=loader)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


class TestEngineAmalgamation(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.build = load_build()
        cls.text = cls.build.amalgamate()

    def test_amalgamation_is_valid_python(self):
        ast.parse(self.text)  # SyntaxError => a module split badly

    def test_single_future_import(self):
        self.assertEqual(
            self.text.count("from __future__ import annotations"), 1,
            "the __future__ import must appear exactly once, in the header",
        )

    def test_no_leaked_relative_imports(self):
        # Every `from .x import y` in a module is editor scaffolding that the
        # build must strip; none may survive into the flat single file.
        tree = ast.parse(self.text)
        leaks = [n.module for n in ast.walk(tree)
                 if isinstance(n, ast.ImportFrom) and n.level and n.level > 0]
        self.assertEqual(leaks, [], f"relative imports leaked into ./trck: {leaks}")

    def test_matches_engine_on_disk(self):
        engine = (REPO_ROOT / "trck").read_text()
        self.assertEqual(
            engine, self.text,
            "./trck is out of sync with src/trck/ — run `python3 build.py`",
        )

    def test_build_is_deterministic(self):
        self.assertEqual(self.text, self.build.amalgamate())


if __name__ == "__main__":
    unittest.main()
