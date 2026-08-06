from __future__ import annotations
from datetime import datetime, timezone
from pathlib import Path
from typing import NoReturn
import os
import re
import sys

__version__ = "0.25.1"
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
DAY_ONLY_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")


def die(msg: str) -> NoReturn:
    print(f"error: {msg}", file=sys.stderr)
    sys.exit(1)


def now_utc() -> str:
    """The stamp written to `created`/`started`/`closed`.

    `TRCK_NOW` overrides the clock. Every verb that records a date goes through here, so
    setting it makes a sequence of commands reproducible — which is what the conformance
    fixtures need, since otherwise any expectation covering `index.jsonl` compares
    against a value that changes every run. Read per call rather than cached, so a
    fixture can advance the clock between invocations and assert the difference.

    Any ISO-8601 instant is accepted and normalised to the one shape the engine writes.
    A day-only value is refused: those are a legacy form the engine no longer emits, and
    expanding one to midnight would reintroduce them through the back door. A malformed
    value is refused rather than ignored — falling back to the real clock would make a
    fixture pass locally and fail elsewhere for a reason nothing in the output explains.

    The Rust engine implements this too; it is part of the conformance contract, not a
    Python-side test hook."""
    override = os.environ.get("TRCK_NOW")
    if not override:
        return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
    if DAY_ONLY_RE.match(override):
        die(f"TRCK_NOW={override!r} is a date, not an instant "
            f"(want e.g. 2026-01-01T00:00:00Z)")
    try:
        dt = datetime.fromisoformat(override)
    except ValueError:
        die(f"TRCK_NOW={override!r} is not an ISO-8601 instant "
            f"(want e.g. 2026-01-01T00:00:00Z)")
    if dt.tzinfo is None:
        dt = dt.replace(tzinfo=timezone.utc)
    return dt.astimezone(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


def date_slice(ts: str | None) -> str:
    return ts[:10] if ts else ""


def slugify(text: str) -> str:
    return re.sub(r"[^a-z0-9]+", "-", text.lower()).strip("-")


