"""Shared test helpers: load the extensionless `trck` engine and build temp trackers."""
import importlib.util
import json
import os
from argparse import Namespace
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
TRCK_PATH = REPO_ROOT / "trck"


def load_trck():
    """Import the extensionless ./trck file as a fresh module object."""
    import importlib.machinery
    loader = importlib.machinery.SourceFileLoader("trck_engine", str(TRCK_PATH))
    spec = importlib.util.spec_from_file_location("trck_engine", TRCK_PATH, loader=loader)
    mod = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(mod)
    return mod


def make_tracker(tmp_path, config=None):
    """Create a tracker dir with a trck.json under tmp_path; return its Path."""
    d = Path(tmp_path) / "issues"
    d.mkdir(parents=True, exist_ok=True)
    if config is None:
        config = {}  # empty -> engine falls back to DEFAULT_CONFIG
    (d / "trck.json").write_text(json.dumps(config))
    return d


def ns(**kwargs):
    """Build an argparse-style Namespace for calling cmd_* handlers directly."""
    return Namespace(**kwargs)
