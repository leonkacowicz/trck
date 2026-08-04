#!/usr/bin/env python3
"""Convert a trck tracker's legacy integer ids to random alphanumeric ids.

Integer ids were trck's first iteration. They were dropped, not migrated: the
engine no longer reads, writes or resolves them, and refuses a tracker that still
has them. This script is the way out, and it is deliberately outside the engine —
a one-shot conversion is not something a tool should carry forever.

    python3 scripts/renumber.py [TRACKER_DIR] [--dry-run]

TRACKER_DIR defaults to "issues". Rewrites index.jsonl, renames the issue body
files, and rewrites parent/depends_on through the map. Also writes
`legacy-ids.json` beside the index: `{"24": "k3m9x2a", ...}`.

**Keep that map.** The engine used to store each issue's old number in a
`legacy_id` field and resolve `trck show 24` through it. It no longer does, so
the map is the only way to read a `#24` in an old commit message. Nothing rewrites
commit history for you.

`#NN` references in issue *bodies* are rewritten, since those are yours to edit.

Standard library only, and it does not import the engine — so it runs against a
tracker that the installed engine already refuses.
"""
import argparse
import json
import re
import secrets
import sys
from pathlib import Path

# Kept in sync with the engine by hand — this file deliberately does not import it.
ID_ALPHABET = "23456789abcdefghjkmnpqrstuvwxyz"  # base32 minus look-alikes
ID_LEN = 7
FILENAME_RE = re.compile(r"^([0-9a-z]+)-([a-z0-9][a-z0-9-]*)\.md$")


def is_legacy(iid) -> bool:
    """An integer id, as either a JSON number or an all-digit string. A random id
    can be all digits too (`2345678` is a legal draw), so length decides."""
    s = str(iid)
    return s.isdigit() and len(s) != ID_LEN


def mint(taken: set) -> str:
    while True:
        cand = "".join(secrets.choice(ID_ALPHABET) for _ in range(ID_LEN))
        if cand not in taken:
            taken.add(cand)
            return cand


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(
        description="Convert a trck tracker's legacy integer ids to random ids.")
    ap.add_argument("tracker_dir", nargs="?", default="issues")
    ap.add_argument("--dry-run", action="store_true",
                    help="print the mapping and planned renames, write nothing")
    args = ap.parse_args(argv)

    d = Path(args.tracker_dir)
    index = d / "index.jsonl"
    if not index.is_file():
        sys.exit(f"error: {index} not found")

    rows = [json.loads(l) for l in index.read_text().splitlines() if l.strip()]
    legacy = [r for r in rows if is_legacy(r.get("id"))]
    if not legacy:
        print("nothing to convert: no integer ids found")
        return 0

    taken = {str(r["id"]) for r in rows if not is_legacy(r.get("id"))}
    mapping = {str(r["id"]): mint(taken) for r in legacy}

    # Bodies live in items/ (flat) — or, in a pre-0.23 tracker, under per-status
    # folders. One pass finds each file wherever it sits. The id in a legacy
    # filename is zero-padded (`024-slug.md`), so it is compared un-padded.
    renames = []
    for p in sorted(d.rglob("*.md")):
        m = FILENAME_RE.match(p.name)
        if not m or not m.group(1).isdigit():
            continue
        old = str(int(m.group(1)))
        if old in mapping:
            renames.append((p, p.with_name(f"{mapping[old]}-{m.group(2)}.md")))

    ref_re = re.compile(r"#(\d{1,6})\b")

    def rewrite_refs(text):
        """Rewrite `#NN` mentions we have a mapping for; leave any other number
        alone rather than guessing at it."""
        return ref_re.sub(
            lambda m: "#" + mapping[m.group(1)] if m.group(1) in mapping else m.group(0),
            text)

    if args.dry_run:
        for old, new in sorted(mapping.items(), key=lambda kv: int(kv[0])):
            print(f"#{old} -> #{new}")
        for src, dst in renames:
            print(f"rename {src.name} -> {dst.name}")
        print(f"({len(mapping)} ids, {len(renames)} files; nothing written)")
        return 0

    for r in rows:
        if is_legacy(r.get("id")):
            r["id"] = mapping[str(r["id"])]
        if r.get("parent") is not None and str(r["parent"]) in mapping:
            r["parent"] = mapping[str(r["parent"])]
        r["depends_on"] = [mapping.get(str(x), str(x)) for x in (r.get("depends_on") or [])]
        if not r["depends_on"]:
            r.pop("depends_on", None)
        r.pop("legacy_id", None)  # the engine no longer has this field
    index.write_text("".join(json.dumps(r, separators=(", ", ": ")) + "\n" for r in rows))

    for src, dst in renames:
        src.rename(dst)
    for p in d.rglob("*.md"):
        text = p.read_text()
        new = rewrite_refs(text)
        if new != text:
            p.write_text(new)

    out = d / "legacy-ids.json"
    out.write_text(json.dumps(
        {k: mapping[k] for k in sorted(mapping, key=int)}, indent=2) + "\n")
    print(f"converted {len(mapping)} issue(s); map written to {out}")
    print("keep that file — a #NN in an old commit message resolves nowhere else")
    return 0


if __name__ == "__main__":
    sys.exit(main())
