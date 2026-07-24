"""Test package bootstrap.

The engine (`./trck`) is a *generated* artifact — the source of truth is the
`src/trck/` package. So before any test runs, regenerate `./trck` from `src/` so
the suite always exercises the current source. This runs once, when the test
package is first imported (under both `unittest discover` and `pytest`).

`build.py` works purely on text (read → ast-strip imports → concatenate); it never
imports the `src/trck/` package, so the package's intentional import cycles are
irrelevant here. The rebuild is byte-exact when things are in sync, so it causes no
churn; it only rewrites (and says so) when `src/` has moved ahead of `./trck`.
"""
import importlib.machinery
import importlib.util
from pathlib import Path

_REPO_ROOT = Path(__file__).resolve().parent.parent


def _rebuild_engine() -> None:
    loader = importlib.machinery.SourceFileLoader("trck_build", str(_REPO_ROOT / "build.py"))
    spec = importlib.util.spec_from_file_location("trck_build", _REPO_ROOT / "build.py", loader=loader)
    build = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(build)

    engine = _REPO_ROOT / "trck"
    fresh = build.amalgamate()
    if not engine.exists() or engine.read_text() != fresh:
        build.write(engine, fresh)
        print("tests: regenerated ./trck from src/trck/ (remember to commit it)")


_rebuild_engine()
