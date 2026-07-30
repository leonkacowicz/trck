from __future__ import annotations
from dataclasses import dataclass, field, fields
from pathlib import Path
import json
import re
from .config import load_config, resolve_tracker_dir, resolve_tracker_dir_or_die
from .constants import FIELD_KEY_RE, ITEMS_DIR, die

# --------------------------------------------------------------------------- #
# context + index I/O
# --------------------------------------------------------------------------- #
CANON_KEYS = [
    "id", "slug", "title", "kind", "status", "priority", "points", "parent",
    "labels", "depends_on", "spec", "pr", "created", "started", "closed",
    "resolution", "manual_status", "legacy_id",
]

# Per-field defaults for the optional trck-owned fields. A field whose value
# equals its default is omitted from the serialized index (noise reduction);
# everything else — required fields without a default, and custom/unknown keys —
# is always written verbatim. The default is per-field, so the test is
# equals-the-default, not falsiness (e.g., a future `points` default of 1 would
# strip `points: 1` while keeping `points: 0`).
DEFAULT_POINTS = 1
FIELD_DEFAULTS = {
    "points": DEFAULT_POINTS,
    "parent": None,
    "labels": [],
    "depends_on": [],
    "spec": None,
    "pr": None,
    "started": None,
    "closed": None,
    "resolution": None,
    "manual_status": False,
    "legacy_id": None,
}


def check_field_key(key: str) -> str | None:
    """Validate a custom-field key. Returns an error message, or None if OK.
    Custom fields are free-form, but their keys must be slug-like and must not
    collide with a built-in field name (use the matching flag/verb for those)."""
    if key in CANON_KEYS:
        return f"'{key}' is a built-in field; use its flag/verb, not --field/--unset"
    if not FIELD_KEY_RE.match(key):
        return f"invalid field key '{key}' (must match [a-z][a-z0-9_-]*)"
    return None


@dataclass
class Issue:
    """The single in-memory representation of an issue.

    The six identity/state fields are required and non-optional; the rest carry
    canonical defaults. Any unrecognized keys from index.jsonl are preserved
    verbatim in `extra` (so custom/forward-compatible fields survive a round-trip).
    The shape, defaults, and (de)serialization all live here — `from_dict` parses
    an index row, `to_canonical` writes the slim ordered form, `to_dict` is the
    full mapping.

    `from_dict` enforces the structural/type contract and fails loud (no guessing,
    no recovery): a row missing a required field or carrying a wrong-typed value
    is not a well-formed issue. Value/config/graph consistency (status in the
    configured vocabulary, points >= 0, parent exists, …) is left to `validate`.
    """
    id: str
    slug: str
    title: str
    kind: str
    status: str
    priority: str
    points: int = 1
    parent: str | None = None
    labels: list[str] = field(default_factory=list)
    depends_on: list[str] = field(default_factory=list)
    spec: str | None = None
    pr: str | None = None
    created: str | None = None
    started: str | None = None
    closed: str | None = None
    resolution: str | None = None
    manual_status: bool = False
    legacy_id: int | None = None
    extra: dict = field(default_factory=dict)

    def __post_init__(self):
        # Ids are opaque strings. Coerce legacy integer ids (and the int ids that
        # tests/handlers still pass) to str at the single construction choke point,
        # so id/parent/depends_on are uniformly typed everywhere downstream.
        if self.id is not None:
            self.id = str(self.id)
        if self.parent is not None:
            self.parent = str(self.parent)
        self.depends_on = [str(d) for d in (self.depends_on or [])]

    @classmethod
    def from_dict(cls, d: dict) -> Issue:
        """Parse one index row into an Issue, enforcing the structural contract.
        Migrates the legacy `milestone` field to a plain label, routes unknown
        keys into `extra`, and raises ValueError on a missing required field or a
        wrong-typed value (load_index turns that into a `line N: …` failure)."""
        if not isinstance(d, dict):
            raise ValueError(f"expected a JSON object, got {type(d).__name__}")
        d = dict(d)
        ms = d.pop("milestone", None)  # legacy field -> migrate to a plain label
        if ms:
            labels = list(d.get("labels") or [])
            if ms not in labels:
                labels.append(ms)
            d["labels"] = labels

        def bad(name, msg):
            raise ValueError(f"field {name!r} {msg}")

        def want_int(name, v):
            if not isinstance(v, int) or isinstance(v, bool):
                bad(name, f"must be an integer, got {v!r}")

        def want_id(name, v):
            if isinstance(v, bool) or not isinstance(v, (int, str)):
                bad(name, f"must be a string or integer id, got {v!r}")
            if isinstance(v, str) and not v:
                bad(name, "must not be empty")

        for k in ("id", "slug", "title", "kind", "status", "priority"):
            if d.get(k) is None:
                bad(k, "is required")
        want_id("id", d["id"])
        for k in ("slug", "title", "kind", "status", "priority"):
            if not isinstance(d[k], str):
                bad(k, f"must be a string, got {d[k]!r}")
        if "points" in d:
            want_int("points", d["points"])
        if d.get("parent") is not None:
            want_id("parent", d["parent"])
        for k in ("labels", "depends_on"):
            if k in d and d[k] is not None and not isinstance(d[k], list):
                bad(k, f"must be a list, got {d[k]!r}")
        for lab in (d.get("labels") or []):
            if not isinstance(lab, str):
                bad("labels", f"must contain only strings, got {lab!r}")
        for dep in (d.get("depends_on") or []):
            want_id("depends_on", dep)
        for k in ("spec", "pr", "created", "started", "closed", "resolution"):
            if d.get(k) is not None and not isinstance(d[k], str):
                bad(k, f"must be a string, got {d[k]!r}")
        if "manual_status" in d and not isinstance(d["manual_status"], bool):
            bad("manual_status", f"must be a boolean, got {d['manual_status']!r}")
        if d.get("legacy_id") is not None:
            want_int("legacy_id", d["legacy_id"])

        known = {k: v for k, v in d.items() if k in CANON_KEYS}
        extra = {k: v for k, v in d.items() if k not in CANON_KEYS}
        return cls(**known, extra=extra)

    def to_dict(self) -> dict:
        """The full mapping: every known field (canonical order) plus extras."""
        return {**{k: getattr(self, k) for k in CANON_KEYS}, **self.extra}

    def to_canonical(self) -> dict:
        """The slim, ordered form written to index.jsonl: known keys in canonical
        order with defaults stripped, then unknown keys appended in stable order.
        A known field equal to its default (or a None required field) is omitted."""
        ordered = {}
        for k in CANON_KEYS:
            v = getattr(self, k)
            if k in FIELD_DEFAULTS:
                if v == FIELD_DEFAULTS[k]:
                    continue  # equals default -> strip as noise
                ordered[k] = v
            elif v is not None:
                ordered[k] = v  # required field, kept when present
        for k in sorted(self.extra):  # unknown keys verbatim, in stable order
            ordered[k] = self.extra[k]
        return ordered


def get_id(issue: Issue) -> str:
    """The unique id of an issue. To be used as the key argument to sort."""
    return issue.id

# CANON_KEYS must list exactly the known Issue fields (the serialization order).
assert CANON_KEYS == [f.name for f in fields(Issue) if f.name != "extra"]


class Ctx:
    """Resolved invocation context: the tracker dir and its merged config."""
    def __init__(self, d: Path, cfg: dict):
        self.dir = Path(d)
        self.cfg = cfg

    @property
    def index_path(self) -> Path:
        return self.dir / "index.jsonl"


def build_ctx(args, required: bool = True) -> Ctx | None:
    d = resolve_tracker_dir(getattr(args, "dir", None), required=required)
    if d is None:
        return None
    return Ctx(d, load_config(d))


def build_ctx_or_die(args) -> Ctx:
    d = resolve_tracker_dir_or_die(getattr(args, "dir", None))
    return Ctx(d, load_config(d))

def file_id(m: re.Match) -> str:
    """The issue id from a FILENAME_RE match. A legacy zero-padded numeric name
    (064) normalizes to its bare integer string (64) so it matches the index's
    coerced string id; a random alphanumeric id is returned unchanged."""
    g = m.group(1)
    return str(int(g)) if g.isdigit() else g


def filename(row: Issue) -> str:
    # Numeric ids keep the historical 3-wide zero padding (so existing on-disk
    # names are byte-identical and `check` stays green with no rename); random
    # alphanumeric ids are written bare.
    head = f"{int(row.id):03d}" if row.id.isdigit() else row.id
    return f"{head}-{row.slug}.md"


def rel_link(row: Issue) -> str:
    return f"{ITEMS_DIR}/{filename(row)}"


def issue_path(ctx: Ctx, row: Issue) -> Path:
    return ctx.dir / ITEMS_DIR / filename(row)


def load_index(ctx: Ctx) -> list[Issue]:
    if not ctx.index_path.exists():
        return []
    rows = []
    for n, line in enumerate(ctx.index_path.read_text().splitlines(), 1):
        line = line.strip()
        if not line:
            continue
        try:
            raw = json.loads(line)
        except json.JSONDecodeError as e:
            die(f"index.jsonl line {n}: invalid JSON ({e})")
        try:
            rows.append(Issue.from_dict(raw))
        except ValueError as e:
            die(f"index.jsonl line {n}: {e}")
    return rows


def save_index(ctx: Ctx, rows: list[Issue]) -> None:
    rows = sorted(rows, key=get_id)
    lines = [json.dumps(r.to_canonical(), ensure_ascii=False) for r in rows]
    ctx.index_path.write_text("\n".join(lines) + ("\n" if lines else ""))


def resolve_ref(rows: list[Issue], token) -> Issue:
    """Resolve a CLI id token to exactly one issue, git-short-hash style. Tiers,
    tried in order; the first tier with matches decides (one match wins, more than
    one is ambiguous): (1) exact id, (2) exact legacy_id (numeric token only),
    (3) unique id prefix. `die`s on no match or an ambiguous reference, listing
    the candidates."""
    token = str(token)
    if token.startswith("#"):
        token = token[1:]  # ids print as "#abc1234"; tolerate a pasted-back "#"
    exact = [r for r in rows if r.id == token]
    if exact:
        return exact[0]
    if token.isdigit():
        legacy = [r for r in rows if r.legacy_id == int(token)]
        if len(legacy) == 1:
            return legacy[0]
        if len(legacy) > 1:
            cands = ", ".join(sorted(r.id for r in legacy))
            die(f"ambiguous legacy id '{token}' matches: {cands}")
    pref = [r for r in rows if r.id.startswith(token)]
    if len(pref) == 1:
        return pref[0]
    if len(pref) > 1:
        cands = ", ".join(sorted(r.id for r in pref))
        die(f"ambiguous id prefix '{token}' matches: {cands}")
    die(f"no issue matching '{token}'")


def unique_prefix_lens(ids) -> dict:
    """Map each id to the length of its shortest prefix that uniquely identifies it
    among `ids` (git-short-hash style: the fewest characters you'd have to type for
    `resolve_ref` to land on it by prefix). When an id is itself a prefix of another
    (e.g. '1' vs '10'), no shorter unique prefix exists, so its full length is used.
    Duplicates in the input collapse; a lone id needs one character."""
    uniq = sorted(set(ids))

    def shared(a: str, b: str) -> int:
        n = 0
        for ca, cb in zip(a, b):
            if ca != cb:
                break
            n += 1
        return n

    out = {}
    for i, s in enumerate(uniq):
        left = shared(s, uniq[i - 1]) if i > 0 else 0
        right = shared(s, uniq[i + 1]) if i + 1 < len(uniq) else 0
        # one char past the longest prefix shared with a sorted neighbour (the only
        # ids that can share a long prefix are adjacent once sorted), capped at len.
        out[s] = min(max(left, right) + 1, len(s))
    return out


def get_row(rows: list[Issue], issue_id) -> Issue:
    return resolve_ref(rows, issue_id)
