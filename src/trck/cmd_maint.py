from __future__ import annotations
from pathlib import Path
import argparse
import ast
import json
import os
import re
import secrets
import shutil
import subprocess
import sys
from .cmd_mutate import cmd_mv
from .config import DEFAULT_CONFIG, DONE, IN_REVIEW, ONGOING, check_review_url, detect_legacy_layout, is_terminal, resolve_tracker_dir
from .constants import DEFAULT_UPDATE_REPO, FILENAME_RE, ID_ALPHABET, ID_LEN, ITEMS_DIR, SELF_PATH, SINCE_RE, __version__, die
from .finalize import finalize
from .graph import Graph, _existing_ids
from .index import build_ctx, build_ctx_or_die, file_id, get_id, issue_path, load_index
from .scan import validate
from .merge import conflict_ids, merge_rows
from .summary import generate_summary, write_summary
from .templates import CLAUDE_MD_TEMPLATE, README_TEMPLATE

def cmd_check(args) -> None:
    ctx = build_ctx_or_die(args)
    rows = load_index(ctx)
    errors, warnings = validate(ctx, rows)
    for w in warnings:
        print(f"warning: {w}")
    for e in errors:
        print(f"error: {e}")
    if errors:
        print(f"\n{len(errors)} error(s), {len(warnings)} warning(s) — FAIL")
        sys.exit(1)
    print(f"OK — {len(rows)} issues, 0 errors, {len(warnings)} warning(s)")


def cmd_summary(args) -> None:
    ctx = build_ctx_or_die(args)
    write_summary(ctx)
    print(f"wrote {ctx.dir / 'SUMMARY.md'} ({len(load_index(ctx))} issues)")


def cmd_normalize(args) -> None:
    ctx = build_ctx_or_die(args)
    rows = load_index(ctx)
    finalize(ctx, rows)  # re-serialize index in canonical slim form + summary + validate
    print(f"normalized {ctx.index_path} ({len(rows)} issues)")


def cmd_install_hook(args) -> None:
    ctx = build_ctx_or_die(args)
    common = subprocess.run(["git", "rev-parse", "--git-common-dir"],
                            cwd=ctx.dir, capture_output=True, text=True)
    toplevel = subprocess.run(["git", "rev-parse", "--show-toplevel"],
                              cwd=ctx.dir, capture_output=True, text=True)
    if common.returncode != 0 or toplevel.returncode != 0:
        die("not a git repository")
    root = Path(toplevel.stdout.strip())
    try:
        rel = ctx.dir.resolve().relative_to(root.resolve()).as_posix()
    except ValueError:
        die(f"tracker dir {ctx.dir} is not inside the git repo at {root}")
    hooks = (ctx.dir / common.stdout.strip()).resolve() / "hooks"
    hooks.mkdir(parents=True, exist_ok=True)
    hook = hooks / "pre-commit"
    # When the tracker dir IS the repo root, rel == "." and a path-prefix grep
    # ('(^|/)\\./') would never match git's repo-relative staged paths — the hook
    # would silently never run. In that case the whole repo is the tracker, so fire
    # on any staged change; otherwise guard on the tracker's path prefix.
    if rel == ".":
        guard = 'if [ -n "$staged" ]; then'
        edir = "$root"
    else:
        rel_re = rel.replace(".", r"\.")
        guard = f"if printf '%s\\n' \"$staged\" | grep -qE '(^|/){rel_re}/'; then"
        edir = f"$root/{rel}"
    body = (
        "#!/usr/bin/env bash\n"
        "# Auto-installed by `trck repo install-hook`. Runs `trck check` when the tracker changes.\n"
        'root="$(git rev-parse --show-toplevel)"\n'
        'staged="$(git diff --cached --name-only)"\n'
        f"{guard}\n"
        f'  if [ -x "{edir}/trck" ]; then\n'
        f'    python3 "{edir}/trck" check || {{ echo "trck inconsistent — aborting commit"; exit 1; }}\n'
        '  elif command -v trck >/dev/null 2>&1; then\n'
        f'    trck --dir "{edir}" check || {{ echo "trck inconsistent — aborting commit"; exit 1; }}\n'
        "  fi\n"
        "fi\n"
    )
    hook.write_text(body)
    hook.chmod(0o755)
    print(f"installed {hook}")



def parse_since(value: str) -> str:
    """Validate a --since cutoff: a bare date (YYYY-MM-DD) or a full UTC
    timestamp (YYYY-MM-DDTHH:MM:SSZ). Returns it unchanged, or dies."""
    if not SINCE_RE.match(value):
        die(f"--since must be a date (YYYY-MM-DD) or timestamp "
            f"(YYYY-MM-DDTHH:MM:SSZ), got {value!r}")
    return value


def select_shipped(cfg: dict, rows: list, since: str) -> list:
    """Issues that 'shipped' on/after `since`: in a terminal status, with a
    `closed` value >= since (plain ISO string compare), and no resolution
    (so wontfix/duplicate/superseded are excluded). All kinds are included."""
    out = []
    for r in rows:
        if not is_terminal(cfg, r.status):
            continue
        if not r.closed or r.closed < since:
            continue
        if r.resolution:
            continue
        out.append(r)
    return out


def render_changelog(cfg: dict, shipped: list, since: str) -> str:
    """Render the shipped set as nested markdown. Issues nest under their
    in-set parent; an issue whose parent is outside the set is a root. Siblings
    (and roots) are ordered by `closed` descending, id ascending on ties. The
    header counts the whole set regardless of nesting depth. Returns the markdown
    string (ends with a single trailing newline)."""
    n = len(shipped)
    header = f"## Shipped since {since} — {n} issue{'s' if n != 1 else ''}"
    if not shipped:
        return f"{header}\n\n_none_\n"

    g = Graph(cfg, shipped)
    out = [header, ""]

    def sib_sorted(items: list) -> list:
        xs = sorted(items, key=lambda r: r.id)            # id ascending
        xs.sort(key=lambda r: (r.closed or ""), reverse=True)  # closed desc, stable
        return xs

    def walk(node, depth: int, seen: set) -> None:
        comp = node.extra.get("component")
        tag = f" ({comp})" if comp else ""
        out.append("  " * depth + f"- #{node.id} {node.title}{tag}")
        if node.id in seen:
            return
        for child in sib_sorted(g.children_of(node)):
            walk(child, depth + 1, seen | {node.id})

    roots = [r for r in shipped if r.parent is None or r.parent not in g.by_id]
    for root in sib_sorted(roots):
        walk(root, 0, set())
    return "\n".join(out) + "\n"


def cmd_changelog(args) -> None:
    ctx = build_ctx_or_die(args)
    since = parse_since(args.since)
    shipped = select_shipped(ctx.cfg, load_index(ctx), since)
    print(render_changelog(ctx.cfg, shipped, since), end="")


def cmd_version(args) -> None:
    print(__version__)
    tracker_dir = resolve_tracker_dir(args.dir, required=False)
    if tracker_dir is not None:
        print(f"tracker: {tracker_dir}", file=sys.stderr)


def cmd_start(args) -> None:
    build_ctx_or_die(args)
    cmd_mv(ns_like(args, status=ONGOING, resolution=None))


def cmd_review(args) -> None:
    """Alias: move to the 'review' status and, given a URL, link the pull request —
    one move, one finalize, one line of output."""
    build_ctx_or_die(args)
    url = getattr(args, "url", None)
    if url is not None and (m := check_review_url(url)):
        die(m)
    cmd_mv(ns_like(args, status=IN_REVIEW, resolution=None, review_url=url))


def cmd_done(args) -> None:
    build_ctx_or_die(args)
    cmd_mv(ns_like(args, status=DONE, resolution=getattr(args, "resolution", None)))


def ns_like(args, **over):
    """Clone an argparse Namespace with overrides (for alias delegation)."""
    data = dict(vars(args))
    data.update(over)
    return argparse.Namespace(**data)


def cmd_init(args) -> None:
    if getattr(args, "target", None) and getattr(args, "init_dir", None):
        die("cannot combine a positional dir with --dir")
    init_dir = getattr(args, "target", None) or getattr(args, "init_dir", None) or "issues"
    target = (Path.cwd() / init_dir).resolve()
    no_vendor = getattr(args, "no_vendor", False)
    if not no_vendor and (target / "trck").resolve() == SELF_PATH:
        die("refusing to vendor over the running engine; use --no-vendor or a different dir")
    cfgfile = target / "trck.json"
    if cfgfile.exists() and not args.force:
        die(f"{target} is already a tracker (pass --force to overwrite config)")
    target.mkdir(parents=True, exist_ok=True)

    config = json.loads(json.dumps(DEFAULT_CONFIG))
    config["update"]["repo"] = DEFAULT_UPDATE_REPO
    cfgfile.write_text(json.dumps(config, indent=2) + "\n")

    if not no_vendor:
        vendored = target / "trck"
        shutil.copyfile(SELF_PATH, vendored)
        shutil.copymode(SELF_PATH, vendored)
        os.chmod(vendored, os.stat(vendored).st_mode | 0o111)

    claude = target / "CLAUDE.md"
    if not claude.exists() or args.force:
        claude.write_text(CLAUDE_MD_TEMPLATE)
    readme = target / "README.md"
    if not readme.exists() or args.force:
        readme.write_text(README_TEMPLATE)

    if args.hook:
        cmd_install_hook(ns_like(args, dir=str(target)))
    print(f"initialized tracker at {target}")


def _update_repo(args) -> str:
    """Resolve which GitHub repo to update from: tracker config if available, else default.

    Skips the format guard: `update` is the remedy a too-new tracker tells you to run,
    so refusing here would leave you with no way to get an engine that understands it.
    Only `update.repo` is read, which is a string in every format."""
    ctx = build_ctx(args, required=False, guard_format=False)  # works outside a tracker too
    if ctx is None:
        return DEFAULT_UPDATE_REPO
    return ctx.cfg.get("update", {}).get("repo") or DEFAULT_UPDATE_REPO


def _current_version() -> str:
    """Read __version__ from the SELF_PATH file (so tests can override SELF_PATH)."""
    text = Path(SELF_PATH).read_text()
    m = re.search(r'^__version__\s*=\s*["\']([^"\']+)["\']', text, re.MULTILINE)
    return m.group(1) if m else __version__


# Scaffolded docs that `init` writes from a template constant. On `update` we
# refresh any copy the user hasn't customized (see _refresh_managed_docs).
MANAGED_DOC_TEMPLATES = {"CLAUDE.md": "CLAUDE_MD_TEMPLATE"}


def _template_literal(source: str, name: str) -> str | None:
    """Extract the value of a top-level `name = "..."` string assignment from engine
    source text, WITHOUT executing it. None if absent or not a plain string literal."""
    try:
        tree = ast.parse(source)
    except SyntaxError:
        return None
    for node in tree.body:
        if isinstance(node, ast.Assign) and any(
            isinstance(t, ast.Name) and t.id == name for t in node.targets
        ):
            if isinstance(node.value, ast.Constant) and isinstance(node.value.value, str):
                return node.value.value
    return None


def _refresh_managed_docs(args, new_source: str) -> None:
    """After the engine is replaced, bring scaffolded docs (CLAUDE.md) up to date.

    A doc is rewritten only when its on-disk copy still equals THIS engine's template
    -- i.e. the user never edited it; a customized copy is reported and left intact.
    Best-effort and tracker-scoped: does nothing when run outside a tracker.

    Note: the "unmodified" test compares against the running engine's template, so a
    copy scaffolded by an older engine (whose template has since changed) reads as
    modified and is left alone -- intentional, to never clobber possible edits."""
    ctx = build_ctx(args, required=False)
    if ctx is None:
        return
    for fname, const in MANAGED_DOC_TEMPLATES.items():
        old = globals().get(const)
        new = _template_literal(new_source, const)
        if not old or not new or new == old:
            continue  # template missing on one side, or unchanged -> nothing to do
        path = ctx.dir / fname
        if not path.exists():
            continue
        if path.read_text() == old:
            path.write_text(new)
            print(f"refreshed {path}")
        else:
            print(f"kept your modified {path} (template changed upstream)")




def cmd_migrate_layout(args) -> None:
    """One-shot: relocate every issue body from its per-status folder into
    `items/`. Status stops being encoded in the path and lives only in
    index.jsonl. Idempotent — a flat tracker is a no-op.

    Deliberately conservative about the one ambiguity a legacy tracker can carry:
    if a file's folder disagrees with its index status, the two sources of truth
    have already drifted and only the author knows which is right, so we stop
    rather than silently canonizing one."""
    ctx = build_ctx_or_die(args, guard_layout=False)
    stale = detect_legacy_layout(ctx.cfg, ctx.dir)
    if not stale:
        print(f"migrate-layout: nothing to migrate (already flat in {ITEMS_DIR}/)")
        return

    rows = load_index(ctx)
    by_id = {r.id: r for r in rows}
    dest_dir = ctx.dir / ITEMS_DIR

    drift, collisions, moves = [], [], []
    for p in stale:
        m = FILENAME_RE.match(p.name)
        iid = file_id(m)
        row = by_id.get(iid)
        if row is not None and row.status != p.parent.name:
            drift.append(f"#{iid}: index says '{row.status}', file sits in "
                         f"'{p.parent.name}/'")
            continue
        dest = dest_dir / p.name
        if dest.exists():
            collisions.append(f"#{iid}: {dest} already exists")
            continue
        moves.append((p, dest))

    if drift:
        detail = "\n  ".join(drift)
        die(f"index status and folder disagree for {len(drift)} issue(s); fix the "
            f"index (or move the files) so they agree, then re-run:\n  {detail}")
    if collisions:
        detail = "\n  ".join(collisions)
        die(f"destination already occupied for {len(collisions)} file(s):\n  {detail}")

    if getattr(args, "dry_run", False):
        print(f"migrate-layout: would move {len(moves)} file(s) into {ITEMS_DIR}/")
        for src, dest in moves:
            print(f"  {src.parent.name}/{src.name} -> {ITEMS_DIR}/{dest.name}")
        return

    dest_dir.mkdir(parents=True, exist_ok=True)
    for src, dest in moves:
        shutil.move(str(src), str(dest))

    # Drop the status folders that are now empty. A folder holding anything else
    # (a README, a scratch note) is left alone — rmdir refuses a non-empty dir.
    for folder in {src.parent for src, _ in moves}:
        try:
            folder.rmdir()
        except OSError:
            pass

    finalize(ctx, rows)  # rewrite SUMMARY.md with items/ links, then validate
    print(f"migrate-layout: moved {len(moves)} file(s) into {ITEMS_DIR}/")


def _read_jsonl(path) -> list:
    """Parse one of git's merge operands. A missing or empty side is legitimate —
    it means the file did not exist in that revision."""
    p = Path(path)
    if not p.exists():
        return []
    rows = []
    for n, line in enumerate(p.read_text().splitlines(), 1):
        line = line.strip()
        if not line:
            continue
        try:
            rows.append(json.loads(line))
        except json.JSONDecodeError as e:
            die(f"{path} line {n}: invalid JSON ({e})")
    return rows


def cmd_merge_index(args) -> None:
    """git merge driver for index.jsonl: a row-wise 3-way merge keyed by id.

    Git passes three temp files — %O (common ancestor), %A, %B — and takes the
    contents of %A as the result. Exit 0 means resolved; non-zero means conflicted.

    The two sides are deliberately NOT named ours/theirs: `%A` is whatever is
    checked out at that moment, so `git merge main` and `git rebase main` from the
    same branch hand them over in opposite order. Every rule in `merge_rows` is
    symmetric or base-derived for that reason.

    On a clean merge this also regenerates SUMMARY.md **from the merged rows** —
    not by re-reading the working-tree index, which during a merge is not yet the
    merged result. That is what makes the driver-ordering question moot: git gives
    no ordering guarantee between per-file drivers, so whichever runs first, the
    rollup ends up derived from the same rows.

    On a conflicted merge it writes conflict markers and leaves SUMMARY.md alone.
    A rollup regenerated from a half-merged index would launder the conflict into a
    plausible-looking file; a stale rollup is obvious, a fabricated one is not."""
    base = _read_jsonl(args.base)
    side_a = _read_jsonl(args.current)
    side_b = _read_jsonl(args.other)
    rows, conflicts = merge_rows(base, side_a, side_b)

    dest = Path(args.current)
    if not conflicts:
        lines = [json.dumps(r.to_canonical(), ensure_ascii=False)
                 for r in sorted(rows, key=get_id)]
        dest.write_text("\n".join(lines) + ("\n" if lines else ""))
        ctx = build_ctx(args, required=False)
        if ctx is not None:
            write_summary(ctx, rows)
        return

    # Conflicted: emit the clean rows plus a marker block per conflicting id, so the
    # file cannot be parsed — and therefore cannot be `git add`ed unread — until a
    # human resolves it. Sides are labelled by position, never by ownership.
    bad = conflict_ids(conflicts)
    by_a = {str(r["id"]): r for r in side_a}
    by_b = {str(r["id"]): r for r in side_b}
    out = [json.dumps(r.to_canonical(), ensure_ascii=False)
           for r in sorted(rows, key=get_id) if r.id not in bad]
    for iid in sorted(bad):
        out.append(f"<<<<<<< one side ({iid})")
        if iid in by_a:
            out.append(json.dumps(by_a[iid], ensure_ascii=False))
        out.append("=======")
        if iid in by_b:
            out.append(json.dumps(by_b[iid], ensure_ascii=False))
        out.append(f">>>>>>> the other side ({iid})")
    dest.write_text("\n".join(out) + "\n")

    print(f"trck: index.jsonl has {len(conflicts)} unresolved conflict(s):",
          file=sys.stderr)
    for c in conflicts:
        print(f"  {c}", file=sys.stderr)
    print("resolve the marked rows, then `git add` and re-run `trck check`.",
          file=sys.stderr)
    sys.exit(1)


def cmd_merge_summary(args) -> None:
    """git merge driver for SUMMARY.md: discard both sides and regenerate.

    The rollup is derived entirely from index.jsonl, so there is never anything to
    merge. This is a safety net rather than the authority — `merge-index` already
    rewrites SUMMARY.md from the rows it merged, which is what makes the order git
    runs the two drivers in irrelevant. If this fires first it regenerates from a
    pre-merge index and `merge-index` corrects it; if it fires second it agrees.

    Regeneration is best-effort: a mid-merge index that does not parse is not an
    error worth failing the whole merge over, and the index driver (or any later
    trck verb) produces the correct rollup anyway."""
    ctx = build_ctx(args, required=False)
    if ctx is None:
        return
    try:
        Path(args.current).write_text(generate_summary(ctx))
    except SystemExit:
        pass  # index mid-merge / unparseable — leave whatever is there


# Matched as a prefix so a header written by an older version is recognised as ours
# and refreshed in place, rather than accumulating one comment per release.
GITATTRIBUTES_HEADER_PREFIX = "# Managed by `trck repo setup-git`"
GITATTRIBUTES_HEADER = (
    GITATTRIBUTES_HEADER_PREFIX
    + " — trck's merge drivers, and the line endings its formats require."
)
# `text eol=lf` is not a style preference. `index.jsonl` and `SUMMARY.md` are
# rendered with `\n` and compared byte for byte, and the bodies are rewritten by
# `edit --title`. Checked out as CRLF, the working tree disagrees with the engine
# from the first verb onwards and every commit shows the whole file as changed.
GITATTRIBUTES_LINES = [
    "index.jsonl merge=trck-index text eol=lf",
    "SUMMARY.md merge=trck-summary text eol=lf",
    "items/*.md text eol=lf",
]


def _gitattributes_update(existing: list[str]) -> list[str] | None:
    """The lines to write, or None when the file already says all of this.

    A line is *ours to replace* when it names one of our paths and carries
    nothing beyond the attributes we manage — which is how a tracker set up
    before an attribute was added gets upgraded in place instead of growing a
    second, stale rule for the same path. A rule carrying anything else is
    somebody's decision, so ours goes beside it and git resolves the pair."""
    out = list(existing)
    changed = False
    missing, last = [], None
    for want in GITATTRIBUTES_LINES:
        pattern, *attrs = want.split()
        ours = set(attrs)
        for i, line in enumerate(out):
            got = line.split()
            if got and got[0] == pattern and set(got[1:]) <= ours:
                if line != want:
                    out[i] = want
                    changed = True
                last = i
                break
        else:
            missing.append(want)

    header_at = next((i for i, ln in enumerate(out)
                      if ln.startswith(GITATTRIBUTES_HEADER_PREFIX)), None)
    if header_at is not None and out[header_at] != GITATTRIBUTES_HEADER:
        out[header_at] = GITATTRIBUTES_HEADER
        changed = True

    if missing:
        changed = True
        if last is not None:
            # Keep the managed block contiguous under the header it already has.
            out[last + 1:last + 1] = missing
        else:
            if out and out[-1].strip():
                out.append("")
            if header_at is None:
                out.append(GITATTRIBUTES_HEADER)
            out.extend(missing)
    return out if changed else None


def _engine_invocation(ctx) -> str:
    """How a git driver should re-invoke this engine. Prefers a vendored copy
    committed beside the tracker (pinned to the data's version, present in CI with
    no install); otherwise re-invokes the engine file running right now.

    Never a bare `trck`: the driver command is baked into .git/config and fires much
    later, from whatever environment git happens to have. A PATH lookup need not
    resolve at all (a CI checkout installs nothing) and, where it does, need not be
    this engine or this version. An absolute path is answerable now."""
    vendored = ctx.dir / "trck"
    if vendored.exists():
        return f'python3 "{vendored.resolve()}"'
    return f'python3 "{Path(SELF_PATH).resolve()}"'


def cmd_setup_git(args) -> None:
    """Declare trck's merge drivers and register them in this clone.

    Two halves, because git separates them on purpose:

    - `<tracker>/.gitattributes` *names* the drivers. Committed, so it is shared.
    - `.git/config` *defines* what they run. Per-clone and never shared — otherwise
      cloning a repo would be remote code execution.

    So this must run once per clone. Until it does, git falls back to an ordinary
    3-way merge with normal conflict markers: an un-set-up clone is exactly as well
    off as before, which is what lets this roll out gradually."""
    ctx = build_ctx_or_die(args)
    common = subprocess.run(["git", "rev-parse", "--git-common-dir"],
                            cwd=ctx.dir, capture_output=True, text=True)
    if common.returncode != 0:
        die("not a git repository")

    # --- shared half: name the drivers ---
    path = ctx.dir / ".gitattributes"
    existing = path.read_text().splitlines() if path.exists() else []
    updated = _gitattributes_update(existing)
    if updated is not None:
        path.write_text("\n".join(updated) + "\n")
        print(f"wrote {path}")
    else:
        print(f"{path} already declares the trck drivers")

    # --- per-clone half: define what they run ---
    engine = _engine_invocation(ctx)
    drivers = {
        "trck-index": (f"{engine} repo merge-index %O %A %B",
                       "trck index.jsonl row-wise 3-way merge"),
        "trck-summary": (f"{engine} repo merge-summary %A",
                         "trck SUMMARY.md regeneration"),
    }
    for name, (cmd, label) in drivers.items():
        for key, value in ((f"merge.{name}.driver", cmd), (f"merge.{name}.name", label)):
            r = subprocess.run(["git", "config", key, value], cwd=ctx.dir,
                               capture_output=True, text=True)
            if r.returncode != 0:
                die(f"git config {key} failed: {r.stderr.strip()}")
    print(f"registered merge drivers in this clone ({', '.join(sorted(drivers))})")
    print("note: .gitattributes is shared, but the driver commands are per-clone — "
          "every clone must run `trck repo setup-git` for auto-resolution to apply.")
