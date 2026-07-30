#!/usr/bin/env python3
"""Amalgamate the `src/trck/` package into the single-file `./trck` engine.

`trck` ships and self-updates as ONE standard-library-only file, but that file is
generated — the source of truth is the `src/trck/` package, split into readable
modules along the engine's natural bands. This script flattens the package back
into the canonical single file.

The scheme is deliberately simple so the output is trustworthy:

  * `src/trck/__init__.py` is the verbatim header — shebang, license banner,
    module docstring, `from __future__ import annotations`, and the one canonical
    block of stdlib imports. It is emitted unchanged.
  * Every other module contributes its body with all *top-level* imports removed.
    Modules carry sibling/stdlib imports only so editors can resolve symbols; the
    amalgamated file is a single flat namespace with one import block (the header),
    so per-module imports are redundant there.

Because the engine already has no top-level imports below its header, every
top-level import in a module body is one added for the editor, so stripping them
all recovers the engine text. Inter-band spacing is owned here (see `amalgamate`),
not inherited from each module's trailing blank lines — so a formatter that trims
those can't cramp one band's header against the previous one. `build.py --check`
verifies the committed ./trck matches this canonical output byte-for-byte.

Usage:
  python3 build.py            # write ./trck from src/trck/ (and chmod +x)
  python3 build.py -o PATH    # write to a different path
  python3 build.py --check    # build in memory; exit 1 (with a diff) if ./trck differs
"""
from __future__ import annotations

import argparse
import ast
import difflib
import os
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent
SRC = REPO_ROOT / "src" / "trck"
DEFAULT_OUT = REPO_ROOT / "trck"

# Concatenation order. `__init__` (the header) is prepended verbatim and is not
# listed here; every entry below is a module whose imports are stripped before it
# is appended. Order is load-bearing: module-level constants execute at import
# time, so a name must be defined before a later module's top-level code uses it.
# This order mirrors the engine's original band layout, which already satisfies
# that constraint.
MANIFEST = [
    "constants",
    "config",
    "index",
    "merge",
    "graph",
    "scan",
    "render",
    "summary",
    "finalize",
    "net",
    "templates",
    "cmd_mutate",
    "cmd_query",
    "cmd_maint",
    "cmd_selfmgmt",
    "cli",
]


def strip_imports(text: str) -> str:
    """Return `text` with every top-level import statement removed.

    Uses `ast` rather than a line regex so that lines beginning with `import` or
    `from` *inside* string literals (the engine embeds Markdown/README templates
    with such prose) are never mistaken for real imports. After the imports are
    dropped, the *single* blank line conventionally written between them and the
    body is removed — but only one, so a band slice that genuinely begins with a
    blank line keeps it, and the surviving body is byte-for-byte the original.
    """
    tree = ast.parse(text)
    drop: set[int] = set()
    for node in tree.body:
        if isinstance(node, (ast.Import, ast.ImportFrom)):
            end = node.end_lineno or node.lineno
            drop.update(range(node.lineno, end + 1))
    lines = text.splitlines(keepends=True)
    kept = [line for i, line in enumerate(lines, start=1) if i not in drop]
    if kept and kept[0].strip() == "":
        kept.pop(0)
    return "".join(kept)


BAND_GAP = "\n\n\n"  # two blank lines between the header and each module


def _strip_blank_lines(text: str) -> str:
    """Drop leading and trailing blank lines, preserving internal spacing."""
    lines = text.split("\n")
    while lines and not lines[0].strip():
        lines.pop(0)
    while lines and not lines[-1].strip():
        lines.pop()
    return "\n".join(lines)


def amalgamate(src: Path = SRC, manifest: list[str] = MANIFEST) -> str:
    """Flatten the package into the single-file engine source text.

    The build owns inter-band spacing: each part (the header, then each import-
    stripped module) is trimmed of its own leading/trailing blank lines, and the
    parts are rejoined with a fixed two-blank-line gap. So however many trailing
    blanks a module happens to carry — none, after a formatter's whitespace trim —
    the bands stay evenly separated in the generated engine."""
    parts = [(src / "__init__.py").read_text()]
    for mod in manifest:
        parts.append(strip_imports((src / f"{mod}.py").read_text()))
    return BAND_GAP.join(_strip_blank_lines(p) for p in parts) + "\n"


def write(out: Path, text: str) -> None:
    """Write `text` to `out` and make it executable (the engine is run directly)."""
    out.write_text(text)
    out.chmod(out.stat().st_mode | 0o111)


def check(out: Path, text: str) -> int:
    """Compare a freshly built engine against `out`; print a diff and return 1 on drift."""
    if not out.exists():
        print(f"{out} does not exist; run `python3 build.py` to generate it.", file=sys.stderr)
        return 1
    current = out.read_text()
    if current == text:
        return 0
    diff = difflib.unified_diff(
        current.splitlines(keepends=True),
        text.splitlines(keepends=True),
        fromfile=f"{out} (on disk)",
        tofile="build(src/trck/) (expected)",
    )
    sys.stderr.writelines(diff)
    print(
        f"\n{out} is out of sync with src/trck/. "
        f"Run `python3 build.py` to regenerate it.",
        file=sys.stderr,
    )
    return 1


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description="Amalgamate src/trck/ into the single-file ./trck engine.")
    p.add_argument("-o", "--output", type=Path, default=DEFAULT_OUT, help="output path (default: ./trck)")
    p.add_argument("--check", action="store_true",
                   help="don't write; exit non-zero (with a diff) if the output is out of sync")
    args = p.parse_args(argv)

    text = amalgamate()
    if args.check:
        return check(args.output, text)
    write(args.output, text)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
