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

    def test_header_verbatim_bodies_stripped_and_ordered(self):
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
            "PAT = re.compile('x')\n"
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


class TestSyntheticRoundTrip(unittest.TestCase):
    """The core guarantee: split a file into (header + verbatim body slices with
    editor imports prepended), amalgamate, and get the original bytes back."""

    def test_split_then_amalgamate_reproduces_original(self):
        import tempfile
        original = (
            "#!/usr/bin/env python3\n"
            "from __future__ import annotations\n"
            "import re\n"
            "\n"
            "A = 1\n"
            "B = re.compile('b')\n"
            "\n"
            "def use():\n"
            "    return A + 1\n"
        )
        lines = original.splitlines(keepends=True)
        header = "".join(lines[0:4])       # shebang, __future__, import, blank
        body_a = "".join(lines[4:6])       # A, B
        body_b = "".join(lines[6:])        # blank, def use
        with tempfile.TemporaryDirectory() as d:
            pkg = Path(d) / "trck"
            pkg.mkdir(parents=True)
            (pkg / "__init__.py").write_text(header)
            # Editor imports prepended to each body; the build must strip them back out.
            (pkg / "alpha.py").write_text("from __future__ import annotations\nimport re\n\n" + body_a)
            (pkg / "beta.py").write_text("from __future__ import annotations\n\n" + body_b)
            rebuilt = build.amalgamate(src=pkg, manifest=["alpha", "beta"])
        self.assertEqual(rebuilt, original)


if __name__ == "__main__":
    unittest.main()
