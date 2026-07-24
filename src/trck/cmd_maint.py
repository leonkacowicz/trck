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
from .config import DEFAULT_CONFIG, is_terminal, resolve_alias, resolve_tracker_dir
from .constants import DEFAULT_UPDATE_REPO, ID_ALPHABET, ID_LEN, SELF_PATH, SINCE_RE, __version__, die
from .finalize import finalize
from .graph import Graph, _existing_ids
from .index import build_ctx, build_ctx_or_die, issue_path, load_index
from .scan import validate
from .summary import write_summary
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


def cmd_renumber(args) -> None:
    """One-shot migration: convert every legacy integer id to a fresh random id.
    Rewrites parent/depends_on through the old->new map, records each issue's prior
    integer id in legacy_id, and renames files. Random ids are left untouched, so a
    second run is a no-op. #NN prose mentions in bodies are unchanged but still
    resolve via the legacy_id alias."""
    ctx = build_ctx_or_die(args)
    rows = load_index(ctx)
    legacy = [r for r in rows if r.id.isdigit()]
    if not legacy:
        print("renumber: no legacy integer ids to convert")
        return
    assigned = _existing_ids(ctx)
    mapping = {}
    for r in legacy:
        while True:
            cand = "".join(secrets.choice(ID_ALPHABET) for _ in range(ID_LEN))
            if cand not in assigned:
                assigned.add(cand)
                break
        mapping[r.id] = cand

    # For each legacy row: snapshot its current path, switch it to the new id,
    # then move the file to the new path.
    for r in legacy:
        old = issue_path(ctx, r)
        r.legacy_id = int(r.id)
        r.id = mapping[r.id]
        new = issue_path(ctx, r)
        new.parent.mkdir(parents=True, exist_ok=True)
        if old.resolve() != new.resolve():
            shutil.move(str(old), str(new))
    # Rewrite cross-references across ALL rows (a random-id row may point at a
    # renumbered one).
    for r in rows:
        if r.parent in mapping:
            r.parent = mapping[r.parent]
        r.depends_on = [mapping.get(d, d) for d in r.depends_on]

    finalize(ctx, rows)
    print(f"renumber: converted {len(legacy)} issue(s) to random ids")


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
        "# Auto-installed by `trck install-hook`. Runs `trck check` when the tracker changes.\n"
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
    ctx = build_ctx_or_die(args)
    target = resolve_alias(ctx.cfg, "start")
    if not target:
        die("no 'start' alias configured; use `trck mv <id> <status>`")
    cmd_mv(ns_like(args, status=target, resolution=None))


def cmd_done(args) -> None:
    ctx = build_ctx_or_die(args)
    target = resolve_alias(ctx.cfg, "done")
    if not target:
        die("no 'done' alias configured; use `trck mv <id> <status>`")
    cmd_mv(ns_like(args, status=target, resolution=getattr(args, "resolution", None)))


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
    """Resolve which GitHub repo to update from: tracker config if available, else default."""
    ctx = build_ctx(args, required=False)  # update works outside a tracker (silently)
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


