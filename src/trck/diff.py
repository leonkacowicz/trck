from __future__ import annotations
from dataclasses import dataclass, field
from pathlib import Path
import sys
from .config import status_names
from .constants import ITEMS_DIR, die
from .index import CANON_KEYS, Ctx, Issue, filename, parse_index

# --------------------------------------------------------------------------- #
# diff: the source seam
# --------------------------------------------------------------------------- #
# `trck diff` compares the tracker at two points, and it must not require git.
# Every query and mutate verb is VCS-free; only `install-hook` and `setup-git`
# shell out, and both are explicitly git-flavoured. So the dependency is isolated
# here rather than assumed: everything downstream consumes a `Snapshot` and cannot
# tell where it came from. Git is one provider among several, added on top.
TIMESTAMP_FIELDS = ("created", "started", "closed")
SET_FIELDS = ("labels", "depends_on")
# Compared field-by-field: every known field except the id (the join key) and the
# timestamps (which are the *evidence* for a status change, not a change of their
# own). Set-valued fields are compared as sets, separately.
SCALAR_FIELDS = tuple(k for k in CANON_KEYS
                      if k != "id" and k not in TIMESTAMP_FIELDS and k not in SET_FIELDS)


class Snapshot:
    """The tracker's state at one point, plus a label for display.

    `body_reader` maps a row to its markdown text, or is None when the source
    cannot supply bodies at all (a bare index.jsonl, stdin). That distinction is
    load-bearing: `body()` returning None means "unavailable from this source",
    which is NOT the same as "" meaning "the body is empty" — reporting the first
    as the second would silently under-report body edits.

    A reader, rather than a directory, is what lets a provider that has no
    directory to point at (a git revision, say) supply bodies on the same terms.
    Bodies are read lazily, so a diff that never asks for one costs nothing.
    """
    def __init__(self, rows: list[Issue], label: str, body_reader=None):
        self.rows = rows
        self.label = label
        self._read_body = body_reader
        self._by_id = {r.id: r for r in rows}

    @property
    def has_bodies(self) -> bool:
        return self._read_body is not None

    def row(self, iid: str) -> Issue | None:
        return self._by_id.get(iid)

    def body(self, iid: str) -> str | None:
        """The issue's markdown body, or None when unavailable — either because the
        source has no bodies at all, or because this issue has none in it."""
        row = self._by_id.get(iid)
        if self._read_body is None or row is None:
            return None
        return self._read_body(row)


def dir_body_reader(items: Path):
    """Read bodies out of an on-disk `items/` directory."""
    def read(row: Issue) -> str | None:
        p = items / filename(row)
        return p.read_text() if p.exists() else None
    return read


def snapshot_from_text(text: str, label: str) -> Snapshot:
    """A rows-only snapshot from index.jsonl text (stdin, or a blob read by a provider)."""
    return Snapshot(parse_index(text, label), label)


def snapshot_from_index(path: Path, label: str | None = None) -> Snapshot:
    """A rows-only snapshot from an index.jsonl file: no bodies alongside it."""
    path = Path(path)
    label = label or path.name
    if not path.exists():
        die(f"no such file: {path}")
    return Snapshot(parse_index(path.read_text(), label), label)


def snapshot_from_dir(path: Path, label: str | None = None) -> Snapshot:
    """A snapshot from a whole tracker dir — rows and bodies both.

    A dir with no index.jsonl is an empty snapshot, not an error: the tracker not
    existing on one side is a legitimate comparison (everything reads as added).
    """
    path = Path(path)
    label = label or path.name
    index = path / "index.jsonl"
    rows = parse_index(index.read_text(), label) if index.exists() else []
    return Snapshot(rows, label, body_reader=dir_body_reader(path / ITEMS_DIR))


def snapshot_working_tree(ctx: Ctx) -> Snapshot:
    return snapshot_from_dir(ctx.dir, "working tree")


def resolve_source(spec: str | None, ctx: Ctx | None = None) -> Snapshot:
    """Turn a `--from`/`--to` value into a Snapshot.

    `-` reads index.jsonl from stdin, a directory is read as a tracker dir (bodies
    included), any other path as a bare index.jsonl. None means the working tree,
    which needs a resolved `ctx`.
    """
    if spec is None:
        if ctx is None:
            die("no tracker to compare against")
        return snapshot_working_tree(ctx)
    if spec == "-":
        return snapshot_from_text(sys.stdin.read(), "stdin")
    path = Path(spec)
    if path.is_dir():
        return snapshot_from_dir(path)
    return snapshot_from_index(path)


# --------------------------------------------------------------------------- #
# diff: the change model
# --------------------------------------------------------------------------- #
@dataclass
class FieldDelta:
    """A scalar field that differs between the two sides."""
    name: str
    old: object
    new: object


@dataclass
class SetDelta:
    """A set-valued field (labels, depends_on) with its gained and lost members."""
    name: str
    added: list
    removed: list


@dataclass
class Change:
    """What happened to one issue between the two snapshots.

    `kind` is added / removed / modified. `direction` describes a status move
    against the *configured* status order — forward, backward, or lateral when
    either side's status is outside the current vocabulary — and is None when the
    status did not move. Renderers need it because a `done -> ongoing` reopen must
    not read like a `backlog -> ongoing` start.
    """
    id: str
    kind: str
    old: Issue | None
    new: Issue | None
    fields: list[FieldDelta] = field(default_factory=list)
    sets: list[SetDelta] = field(default_factory=list)
    timestamps: dict = field(default_factory=dict)
    direction: str | None = None


@dataclass
class Diff:
    """The change records plus both snapshots — renderers need the full rows for
    titles, icons, and rollups, not just what changed."""
    old: Snapshot
    new: Snapshot
    changes: list[Change]


def status_direction(cfg: dict, old: str, new: str) -> str | None:
    """Classify a status move against the configured order. A status the current
    trck.json doesn't know (an old snapshot written under a renamed vocabulary) is
    unordered, so the move is 'lateral' rather than an error."""
    if old == new:
        return None
    order = status_names(cfg)
    if old not in order or new not in order:
        return "lateral"
    return "forward" if order.index(new) > order.index(old) else "backward"


def _values(row: Issue) -> dict:
    """Every comparable scalar of a row, built-in and custom alike."""
    full = row.to_dict()
    return {k: full.get(k) for k in SCALAR_FIELDS} | dict(row.extra)


def _compare(cfg: dict, old: Issue, new: Issue) -> Change | None:
    """One issue present on both sides: None when nothing moved."""
    ov, nv = _values(old), _values(new)
    fields = [FieldDelta(k, ov.get(k), nv.get(k))
              for k in sorted(ov.keys() | nv.keys()) if ov.get(k) != nv.get(k)]
    sets = []
    for k in SET_FIELDS:
        a, b = set(getattr(old, k) or []), set(getattr(new, k) or [])
        if a != b:
            sets.append(SetDelta(k, sorted(b - a), sorted(a - b)))
    stamps = {k: (getattr(old, k), getattr(new, k))
              for k in TIMESTAMP_FIELDS if getattr(old, k) != getattr(new, k)}
    if not (fields or sets or stamps):
        return None
    return Change(id=new.id, kind="modified", old=old, new=new, fields=fields,
                  sets=sets, timestamps=stamps,
                  direction=status_direction(cfg, old.status, new.status))


def diff_snapshots(cfg: dict, old: Snapshot, new: Snapshot) -> Diff:
    """Join two snapshots by id and classify what changed. Pure: no I/O, no notion
    of a revision — whoever produced the snapshots owns that."""
    olds = {r.id: r for r in old.rows}
    news = {r.id: r for r in new.rows}
    changes = []
    for iid in sorted(olds.keys() | news.keys()):
        o, n = olds.get(iid), news.get(iid)
        if o is None:
            changes.append(Change(id=iid, kind="added", old=None, new=n))
        elif n is None:
            changes.append(Change(id=iid, kind="removed", old=o, new=None))
        elif (c := _compare(cfg, o, n)) is not None:
            changes.append(c)
    return Diff(old=old, new=new, changes=changes)
