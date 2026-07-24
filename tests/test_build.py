"""Unit tests for the amalgamator (`build.py`).

These use small synthetic packages so the build logic is verified independently of
the real 3130-line engine. The whole-engine byte-exact round-trip lives in
`test_build_roundtrip.py` and only makes sense once `src/trck/` exists.
"""
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


build = load_build()


class TestStripImports(unittest.TestCase):
    def test_removes_top_level_imports_and_leading_blank(self):
        text = (
            "from __future__ import annotations\n"
            "import re\n"
            "from .graph import Graph\n"
            "\n"
            "X = re.compile('a')\n"
        )
        self.assertEqual(build.strip_imports(text), "X = re.compile('a')\n")

    def test_preserves_body_verbatim_including_leading_comment(self):
        text = (
            "from __future__ import annotations\n"
            "import json\n"
            "\n"
            "# --- a band separator --- #\n"
            "def f():\n"
            "    return json.dumps({})\n"
        )
        self.assertEqual(
            build.strip_imports(text),
            "# --- a band separator --- #\ndef f():\n    return json.dumps({})\n",
        )

    def test_ignores_import_keywords_inside_string_literals(self):
        # A template string whose prose starts lines with 'import'/'from' must survive.
        text = (
            "from __future__ import annotations\n"
            "\n"
            'TEMPLATE = """\n'
            "import this into your project\n"
            "from here you can edit\n"
            '"""\n'
        )
        expected = (
            'TEMPLATE = """\n'
            "import this into your project\n"
            "from here you can edit\n"
            '"""\n'
        )
        self.assertEqual(build.strip_imports(text), expected)

    def test_body_starting_non_blank_is_untouched_when_no_imports(self):
        text = "Y = 1\nZ = 2\n"
        self.assertEqual(build.strip_imports(text), "Y = 1\nZ = 2\n")


class TestAmalgamate(unittest.TestCase):
    def _make_pkg(self, tmp: Path):
        pkg = tmp / "trck"
        pkg.mkdir(parents=True)
        # Header carries the shebang + the canonical import block, verbatim.
        (pkg / "__init__.py").write_text(
            "#!/usr/bin/env python3\n"
            '"""doc."""\n'
            "from __future__ import annotations\n"
            "import re\n"
            "\n"
        )
        (pkg / "constants.py").write_text(
            "from __future__ import annotations\n"
            "import re\n"
            "\n"
            "PAT = re.compile('x')\n"
        )
        (pkg / "cli.py").write_text(
            "from __future__ import annotations\n"
            "from .constants import PAT\n"
            "\n"
            "def main():\n"
            "    return PAT\n"
            '\n'
            'if __name__ == "__main__":\n'
            "    main()\n"
        )
        return pkg

    def test_header_and_bodies_stripped_ordered_and_evenly_spaced(self):
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            pkg = self._make_pkg(Path(d))
            out = build.amalgamate(src=pkg, manifest=["constants", "cli"])
        expected = (
            "#!/usr/bin/env python3\n"
            '"""doc."""\n'
            "from __future__ import annotations\n"
            "import re\n"
            "\n"
            "\n"
            "PAT = re.compile('x')\n"
            "\n"
            "\n"
            "def main():\n"
            "    return PAT\n"
            "\n"
            'if __name__ == "__main__":\n'
            "    main()\n"
        )
        self.assertEqual(out, expected)

    def test_amalgamated_output_is_valid_python(self):
        import ast as _ast
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            pkg = self._make_pkg(Path(d))
            out = build.amalgamate(src=pkg, manifest=["constants", "cli"])
        _ast.parse(out)  # raises SyntaxError on failure


class TestInterBandSpacing(unittest.TestCase):
    """The build owns inter-band spacing: a module's own trailing blank lines
    (which formatters trim) must not change how far apart the bands sit."""

    def _amalgamate(self, trailing_a: str, trailing_b: str) -> str:
        import tempfile
        with tempfile.TemporaryDirectory() as d:
            pkg = Path(d) / "trck"
            pkg.mkdir(parents=True)
            (pkg / "__init__.py").write_text(
                "#!/usr/bin/env python3\nfrom __future__ import annotations\n")
            (pkg / "alpha.py").write_text(
                "from __future__ import annotations\n\nA = 1" + trailing_a)
            (pkg / "beta.py").write_text(
                "from __future__ import annotations\n\nB = 2" + trailing_b)
            return build.amalgamate(src=pkg, manifest=["alpha", "beta"])

    def test_output_is_independent_of_module_trailing_blanks(self):
        # zero trailing blanks vs several — the amalgamation must be identical
        tight = self._amalgamate("\n", "\n")
        loose = self._amalgamate("\n\n\n\n", "\n\n\n")
        self.assertEqual(tight, loose)

    def test_exactly_two_blank_lines_between_bands(self):
        out = self._amalgamate("\n", "\n")
        self.assertIn("A = 1\n\n\nB = 2\n", out)   # two blank lines between bands
        self.assertTrue(out.endswith("B = 2\n"))    # single trailing newline
        self.assertNotIn("\n\n\n\n", out)           # never more than two


if __name__ == "__main__":
    unittest.main()
