from __future__ import annotations
from datetime import datetime, timezone
from pathlib import Path
from typing import NoReturn
import re
import sys

__version__ = "0.20.0"
DEFAULT_UPDATE_REPO = "leonkacowicz/trck"
SELF_PATH = Path(__file__).resolve()  # the running engine file; tests may override

SLUG_RE = re.compile(r"^[a-z0-9][a-z0-9-]*$")
FILENAME_RE = re.compile(r"^([0-9a-z]+)-([a-z0-9][a-z0-9-]*)\.md$")
ID_ALPHABET = "23456789abcdefghjkmnpqrstuvwxyz"  # base32 minus 0/1/o/l/i, lowercase for typeability
ID_LEN = 7                                        # 31**7 ≈ 2.75e10 id space
ID_RE = re.compile(rf"^[{ID_ALPHABET}]+$")
FIELD_KEY_RE = re.compile(r"^[a-z][a-z0-9_-]*$")
SINCE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}(T\d{2}:\d{2}:\d{2}Z)?$")


def die(msg: str) -> NoReturn:
    print(f"error: {msg}", file=sys.stderr)
    sys.exit(1)


def now_utc() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def date_slice(ts: str | None) -> str:
    return ts[:10] if ts else ""


def slugify(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


