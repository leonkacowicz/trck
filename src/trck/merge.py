from __future__ import annotations
from .index import CANON_KEYS, Issue

# --------------------------------------------------------------------------- #
# row-wise 3-way merge of index.jsonl
# --------------------------------------------------------------------------- #
# A git merge driver receives three files — %O (common ancestor), %A, %B — and
# CANNOT determine which side is the user's. `%A` is simply whatever is checked
# out at that moment: `git merge main` from a feature branch and `git rebase main`
# from that same branch assign the operands in opposite order. So every rule here
# is either symmetric in (a, b) or derived from the base, and none may branch on
# "ours". The tests assert symmetry by running every case both ways round.
#
# The base is what makes that sufficient: `base -> side` is the transaction that
# produced that side, so "who changed what" is recoverable without knowing whose
# change it was.

# Sets, merged as (base + additions) - removals so a deliberate removal on one
# side is not resurrected by the other side's untouched copy.
SET_FIELDS = ("labels", "depends_on")

# Earliest wins; `min` is commutative, so this is symmetric for free.
MIN_FIELDS = ("created", "started")

# `(status, closed, resolution)` is maintained as a unit by `move_issue`, which
# clears both dates on any move to a non-terminal status. Merging its members
# independently synthesizes rows no verb can write — and does so even when no
# single field diverges, so a per-field rule never catches it. Merge the tuple
# whole or conflict.
TUPLE_FIELDS = ("status", "closed", "resolution")


def _by_id(rows) -> dict:
    return {str(r["id"]): r for r in rows}


def _set_merge(base, a, b) -> list:
    base_s, a_s, b_s = set(base or []), set(a or []), set(b or [])
    added = (a_s - base_s) | (b_s - base_s)
    removed = (base_s - a_s) | (base_s - b_s)
    return sorted((base_s | added) - removed)


def _min_merge(a, b):
    vals = [v for v in (a, b) if v is not None]
    return min(vals) if vals else None


def _scalar_merge(iid, field, base, a, b, conflicts):
    """Standard 3-way: one side changed → take it; both changed alike → fine;
    both changed differently → conflict, and keep the base so the result stays
    symmetric (picking a side would make the output depend on operand order)."""
    if a == b:
        return a
    if a == base:
        return b
    if b == base:
        return a
    conflicts.append(f"#{iid}: {field} is {_pair(a, b)}")
    return base


def _pair(x, y) -> str:
    """Render two competing values in a fixed order. The order must not depend on
    which operand they arrived in: `%A`/`%B` swap between integration directions,
    so an operand-ordered message would read differently for the same underlying
    disagreement — and the wording deliberately avoids ours/theirs for the same
    reason."""
    lo, hi = sorted((repr(x), repr(y)))
    return f"{lo} on one side and {hi} on the other"


def _tuple_of(rowdict) -> tuple:
    return tuple(rowdict.get(f) for f in TUPLE_FIELDS)


def merge_rows(base_rows, a_rows, b_rows) -> tuple[list[Issue], list[str]]:
    """3-way merge two sets of index rows keyed by id.

    Returns `(rows, conflicts)`. A non-empty `conflicts` list means the caller must
    not treat the result as resolved — the rows are still returned (holding base
    values where a field conflicted) so a caller can show context, but the merge
    has failed. Messages never say ours/theirs: those words mean opposite things
    depending on the integration direction, so they would be wrong half the time.
    """
    base, a, b = _by_id(base_rows), _by_id(a_rows), _by_id(b_rows)
    conflicts: list[str] = []
    merged: dict[str, dict] = {}

    for iid in sorted(set(a) | set(b)):
        in_base, ra, rb = iid in base, a.get(iid), b.get(iid)
        # Deleted on one side. Honour the deletion if the other side left it
        # alone; a delete-vs-modify is a genuine disagreement.
        if ra is None or rb is None:
            present = ra if ra is not None else rb
            if present is None:
                continue  # unreachable: iid came from a ∪ b, so one side has it
            if in_base and present != base[iid]:
                conflicts.append(f"#{iid}: removed on one side and modified on the other")
                merged[iid] = present
            elif in_base:
                continue  # unchanged on the surviving side -> the deletion wins
            else:
                merged[iid] = present  # created on one side only
            continue
        merged[iid] = _merge_one(iid, base.get(iid, {}), ra, rb, conflicts)

    # A parent's status and points are DERIVED (`normalize_statuses`,
    # `normalize_points`), so a divergence there is not two people disagreeing —
    # it is two sides having recomputed from different child sets. Drop those
    # conflicts; the caller re-derives. Leaves keep the real rule.
    parents = {str(r.get("parent")) for r in merged.values() if r.get("parent")}
    if parents:
        conflicts = [c for c in conflicts
                     if not any(c.startswith(f"#{p}:") and
                                (" status " in c or " points " in c or
                                 "lifecycle" in c)
                                for p in parents)]

    rows = [Issue.from_dict(r) for _, r in sorted(merged.items())]
    return rows, conflicts


def _merge_one(iid, base, a, b, conflicts) -> dict:
    """Merge one row present on both sides, field by field, using `base` to tell
    which side changed what. An absent base means the id was created independently
    on both sides, which makes every differing field a conflict — correct, and
    vanishingly rare with random ids."""
    out = {"id": iid}

    ta, tb, tbase = _tuple_of(a), _tuple_of(b), _tuple_of(base)
    if ta == tb:
        chosen = ta
    elif ta == tbase:
        chosen = tb
    elif tb == tbase:
        chosen = ta
    else:
        # Named by content, not by side: "one side"/"the other" reads correctly
        # whichever direction produced the merge.
        conflicts.append(
            f"#{iid}: lifecycle status is {_pair(ta[0], tb[0])} "
            f"(status/closed/resolution merge as a unit)")
        chosen = tbase
    for field, value in zip(TUPLE_FIELDS, chosen):
        out[field] = value

    for field in CANON_KEYS:
        if field == "id" or field in TUPLE_FIELDS:
            continue
        va, vb, vbase = a.get(field), b.get(field), base.get(field)
        if field in SET_FIELDS:
            out[field] = _set_merge(vbase, va, vb)
        elif field in MIN_FIELDS:
            out[field] = _min_merge(va, vb)
        else:
            out[field] = _scalar_merge(iid, field, vbase, va, vb, conflicts)

    # Custom fields (#h7xp2dm) merge per key with the same scalar rule, so a
    # branch adding `assignee` and another adding `component` keeps both.
    known = set(CANON_KEYS)
    extra_keys = sorted({k for k in (*a, *b, *base) if k not in known})
    for key in extra_keys:
        out[key] = _scalar_merge(iid, f"field {key}", base.get(key),
                                 a.get(key), b.get(key), conflicts)

    return {k: v for k, v in out.items() if v is not None}
