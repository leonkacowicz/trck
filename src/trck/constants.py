from __future__ import annotations
from datetime import datetime, timezone
from pathlib import Path
from typing import NoReturn
import re
import sys

__version__ = "0.25.0"
DEFAULT_UPDATE_REPO = "leonkacowicz/trck"
SELF_PATH = Path(__file__).resolve()  # the running engine file; tests may override

# --------------------------------------------------------------------------- #
# tracker format version
# --------------------------------------------------------------------------- #
# `trck.json` may carry `format: N`. Absent means the current shape, so every
# tracker written before this existed reads as SUPPORTED_FORMAT and nothing breaks.
# An engine refuses a tracker whose format it does not understand; it never refuses
# an older one, which is what migration verbs are for.
#
# Until this shipped, the vendored copy of the engine *was* the pin: the writer
# lived in the repo, so writer and reader could not disagree. That is the whole of
# what vendoring bought, and the reason un-vendoring waits on this.
#
# When to bump — the test is whether an older engine would be *wrong*, not merely
# ignorant:
#
#   no bump     Adding a field to index.jsonl. `Issue.extra` round-trips unknown
#               keys verbatim, so an old engine that reads and rewrites such a row
#               preserves it. Likewise a new verb, flag, or output column.
#   bump        Changing what an existing field means, or where data lives, so that
#               an old engine silently gives wrong answers or destroys data. Both
#               historical breaks would have qualified: status-folders -> items/
#               (an old engine looks for bodies in the wrong place) and integer ->
#               random ids (an old engine's generator collides).
#   extension   An opt-in feature only some trackers use. Bumping `format` for it
#               would lock out old engines for every repo, including the ones not
#               using it; an extension key locks out only those that opted in.
#
# The extension mechanism is git's `extensions.*`, for its granularity: the version
# says "you may meet extension keys — refuse any you do not know". The live example
# that motivated it was a status carrying `actionable: false`, which an engine
# predating that feature read, ignored, and then offered as work in `ready`.
#
# Honest limitation: this protects engines from this release forward. One older
# than it ignores both keys and can still be fooled — which is exactly why the
# vendored copy stays until an installed engine is guaranteed to be newer.
SUPPORTED_FORMAT = 1
KNOWN_EXTENSIONS = frozenset()  # none yet; tests reassign this module global

SLUG_RE = re.compile(r"^[a-z0-9][a-z0-9-]*$")
FILENAME_RE = re.compile(r"^([0-9a-z]+)-([a-z0-9][a-z0-9-]*)\.md$")
ITEMS_DIR = "items"  # the one directory holding every issue body; status lives in index.jsonl
ID_ALPHABET = "23456789abcdefghjkmnpqrstuvwxyz"  # base32 minus 0/1/o/l/i, lowercase for typeability
ID_LEN = 7                                        # 31**7 ≈ 2.75e10 id space
ID_RE = re.compile(rf"^[{ID_ALPHABET}]+$")
FIELD_KEY_RE = re.compile(r"^[a-z][a-z0-9_-]*$")
PR_URL_RE = re.compile(r"^https?://\S+$")  # forge-agnostic: any absolute http(s) link
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


