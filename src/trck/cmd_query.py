from __future__ import annotations
from pathlib import Path
import json
import sys
from .cmd_maint import ns_like
from .constants import FILENAME_RE, date_slice, die
from .diff import Change, diff_snapshots, resolve_source
from .gitsrc import git_snapshot, parse_rev_spec
from .graph import Graph, load_graph
from .index import CANON_KEYS, Ctx, Issue, build_ctx_or_die, check_field_key, file_id, get_id, get_row, issue_path, load_index, resolve_ref, unique_prefix_lens
from .render import block_annotations, demand_annotation, deps_overview_ids, filter_deps_graph_ids, graph_components, hl_id, node_label, paint, paint_lane, parse_status_filter, priority_codes, priority_rank, render_graph, status_codes, status_icon
from .summary import progress_pct

def cmd_show(args) -> None:
    ctx = build_ctx_or_die(args)
    rows = load_index(ctx)
    row = get_row(rows, args.id)
    full = row.to_dict()
    keys = CANON_KEYS + sorted(row.extra)
    if not Graph(ctx.cfg, rows).is_leaf(row):
        keys = [k for k in keys if k != "points"]  # derived from leaves, not an input here
    if getattr(args, "json", False):
        print(json.dumps({k: full.get(k) for k in keys}, ensure_ascii=False, indent=2))
    else:
        abbrev = unique_prefix_lens([r.id for r in rows])
        w = max(len(k) for k in keys)
        for k in keys:
            v = full.get(k)
            if v is None or v == [] or v is False:
                continue  # skip empty fields (and an unset manual_status) in the human view
            if k in ("created", "started", "closed"):
                v = date_slice(v)
            elif k == "id":
                v = hl_id(v, abbrev, hash_prefix=False)  # bold the shortest unique prefix, as in `list`
            print(f"{paint(f'{k:>{w}}', 'dim')}  {v}")
    print("\n--- body ---\n")
    print(issue_path(ctx, row).read_text())


def cmd_path(args) -> None:
    ctx = build_ctx_or_die(args)
    row = get_row(load_index(ctx), args.id)
    print(issue_path(ctx, row).resolve())


def forest_layout(g: Graph, roots, shown: set, key):
    """Depth-first forest over the `shown` id set: returns (ordered_rows, prefix_by_id).
    Roots render with no connector; descendants get `walk_tree`-style branch prefixes.
    Siblings (and roots) are ordered by `key`. Cycle-guarded via `seen`."""
    ordered, prefix_of = [], {}

    def walk(nodes, pfx, seen):
        nodes = [n for n in nodes if n.id in shown]
        for i, n in enumerate(nodes):
            last = i == len(nodes) - 1
            ordered.append(n)
            prefix_of[n.id] = pfx + ("└─ " if last else "├─ ")
            if n.id in seen:
                continue
            ext = "   " if last else "│  "
            walk(g.children_of(n, key=key), pfx + ext, seen | {n.id})

    for root in sorted(roots, key=key):
        ordered.append(root)
        prefix_of[root.id] = ""
        walk(g.children_of(root, key=key), "", {root.id})
    return ordered, prefix_of


def cmd_list(args) -> None:
    ctx = build_ctx_or_die(args)
    g = load_graph(ctx)

    keep_st, drop_st = parse_status_filter(args.status)
    match = (getattr(args, "match", None) or "").lower()
    only_blocked = getattr(args, "blocked", False)
    only_orphan = getattr(args, "orphan", False)
    parent_filter = None if args.parent is None else resolve_ref(g.rows, args.parent).id

    # Default view hides settled work: a terminal issue is dropped unless it is
    # still open *or* sits directly under a non-terminal parent (so an open epic
    # keeps its done children as progress context). `--all` and an explicit
    # `--status` both bypass this prune. The forest's match_closure still pulls
    # open ancestors back as dimmed context, so a done parent of open work shows.
    prune_settled = args.status is None and not getattr(args, "all", False)

    def settled(r):
        if not g.is_terminal(r):
            return False
        parent = g.by_id.get(r.parent) if r.parent is not None else None
        return parent is None or g.is_terminal(parent)

    field_filters = {}
    for spec in (getattr(args, "field", None) or []):
        if "=" not in spec:
            die(f"--field expects key=value, got '{spec}'")
        k, v = spec.split("=", 1)
        if (m := check_field_key(k)):
            die(m)
        field_filters[k] = v
    show_fields = getattr(args, "show_field", None) or []

    def keep(r):
        return ((not keep_st or r.status in keep_st)
                and r.status not in drop_st
                and (not args.kind or r.kind == args.kind)
                and (not args.priority or r.priority == args.priority)
                and (not getattr(args, "label", None) or args.label in (r.labels or []))
                and (parent_filter is None or r.parent == parent_filter)
                and (not match or match in r.title.lower())
                and (not only_blocked or g.is_blocked(r))
                and (not only_orphan or r.parent is None)
                and (not prune_settled or not settled(r))
                and all(r.extra.get(k) == v for k, v in field_filters.items()))

    sort = getattr(args, "sort", None) or "created"
    if sort.startswith("field:"):
        fname = sort[len("field:"):]
        if not fname:
            die("--sort field: needs a field name (e.g. --sort field:assignee)")
        # present rows (group 0) sort by value then id; missing rows (group 1) sort last
        key = lambda r: (0, r.extra[fname], r.id) if fname in r.extra else (1, "", r.id)
    else:
        sort_keys = {
            "priority": lambda r: (priority_rank(ctx.cfg, r.priority), r.id),
            "points": lambda r: (-r.points, r.id),
            "created": lambda r: (r.created or "", r.id),
            "id": get_id,
        }
        if sort not in sort_keys:
            die(f"unknown --sort '{sort}' "
                "(choices: id, priority, points, created, field:NAME)")
        key = sort_keys[sort]
    # the blocking note is view-aware: it spells an inherited dependency out only when
    # the ancestor carrying it isn't among the rows being printed (see
    # `block_annotations`), so each view supplies the set it is about to render.
    annotate_over = lambda rows: (
        lambda r: block_annotations(g, r, {x.id for x in rows}))
    progress = lambda r: progress_pct(g, r)
    # Bold the shortest prefix that uniquely identifies each id across the whole
    # tracker (what you'd type for `show`/`set`/…); the rest of the id is dimmed.
    abbrev = unique_prefix_lens([r.id for r in g.rows])

    if getattr(args, "paths", False):
        for r in sorted((r for r in g.rows if keep(r)), key=key):
            print(issue_path(ctx, r).resolve())
        return

    if getattr(args, "flat", False):
        rows = sorted([r for r in g.rows if keep(r)], key=key)
        print_rows(ctx, rows, annotate=annotate_over(rows), show_fields=show_fields,
                   progress=progress, abbrev=abbrev)
        return

    # nested forest: show a node iff it matches or has a matching descendant; the
    # non-matching ancestors are kept as dimmed context. Siblings sort by `key`.
    shown, dim = g.match_closure(keep)
    root_id = getattr(args, "id", None)
    if root_id is not None:
        root = g.row(root_id)
        roots = [root] if root.id in shown else []
    else:
        roots = [r for r in g.rows
                 if (r.parent is None or r.parent not in g.by_id) and r.id in shown]
    ordered, prefix_of = forest_layout(g, roots, shown, key)
    print_rows(ctx, ordered, annotate=annotate_over(ordered),
               prefix=lambda r: prefix_of[r.id], dim=lambda r: r.id in dim,
               show_fields=show_fields, progress=progress, abbrev=abbrev)


def print_rows(ctx: Ctx, rows: list[Issue], annotate=None, prefix=None, dim=None,
               show_fields=None, progress=None, abbrev=None) -> None:
    """Render issues as aligned one-line summaries (shared by `list` and `ready`).
    `annotate`, if given, maps a row to a trailing suffix (e.g. the blocking graph);
    callers that want terse output (`ready`/`next`) simply omit it. `prefix`, if given,
    maps a row to a connector string placed immediately before the title — empty for the
    flat view, `walk_tree`-style connectors (`├─ `/`└─ `) for the nested forest. `dim`,
    if given, marks rows shown only as ancestor context (in a filtered forest); their
    whole line is dimmed instead of per-field colored. `progress`, if given, maps a row
    to a dim completion suffix placed right after the title (empty for leaves).
    `abbrev`, if given, maps an id to its shortest-unique-prefix length (see
    `unique_prefix_lens`); that prefix is bolded and the rest of the id dimmed
    (git-short-hash style). Without it, the whole id is bolded."""
    if not rows:
        return
    sw = max(len(r.status) for r in rows)
    pw = max(len(r.priority) for r in rows)
    for r in rows:
        pre = prefix(r) if prefix else ""
        prog = progress(r) if progress else ""
        tags = []
        if r.kind == "epic":
            tags.append("EPIC")
        if r.parent and not pre:  # the connector already shows parentage when nested
            tags.append(f"↳{r.parent}")
        tags.extend(r.labels or [])
        plain_tags = " [" + " ".join(tags) + "]" if tags else ""
        ann = annotate(r) if annotate else ""
        fsuf = ""
        if show_fields:
            # to_dict() so a built-in field (pr, spec, …) is showable too, not just
            # a custom one; an unset/empty value contributes no column.
            full = r.to_dict()
            segs = [f"{n}={full[n]}" for n in show_fields
                    if full.get(n) not in (None, "", [], False)]
            if segs:
                fsuf = "  " + paint(" ".join(segs), "dim")
        if dim and dim(r):  # ancestor context: whole line dimmed, no per-field color
            body = f"{status_icon(ctx, r.status)} #{r.id} {r.status:<{sw}}  {r.priority:<{pw}}  {pre}{r.title}{prog}{plain_tags}"
            print(paint(body, "dim") + ann + fsuf)
            continue
        codes = status_codes(ctx.cfg, r.status)
        icon = paint(status_icon(ctx, r.status), *codes)
        iid = hl_id(r.id, abbrev)
        status = paint(f"{r.status:<{sw}}", *codes)
        prio = paint(f"{r.priority:<{pw}}", *priority_codes(ctx.cfg, r.priority))
        progstr = paint(prog, "dim") if prog else ""
        tagstr = paint(plain_tags, "dim") if tags else ""
        print(f"{icon} {iid} {status}  {prio}  {pre}{r.title}{progstr}{tagstr}{ann}{fsuf}")


def change_summary(c: Change) -> str:
    """A compact, plain-text account of what moved on one issue."""
    bits = [f"{f.name} {f.old} → {f.new}" for f in c.fields]
    for s in c.sets:
        members = " ".join([f"+{v}" for v in s.added] + [f"-{v}" for v in s.removed])
        bits.append(f"{s.name} {members}")
    if not bits:  # a timestamp-only edit still changed something; say what
        bits = [f"{k} {a} → {b}" for k, (a, b) in sorted(c.timestamps.items())]
    return ", ".join(bits)


def cmd_diff(args) -> None:
    """Compare the tracker at two points and report what changed.

    A bare revision spec goes through git; `--from`/`--to` name sources directly
    and never touch it. With neither, the default is HEAD vs the working tree —
    "what have I not committed?" — which is the git path too.

    The output here is deliberately minimal — one plain line per changed issue.
    The real layouts (epic rollup, --flat ledger, --stat headline, -v field
    blocks) are separate issues that replace this; it exists so the verb and its
    source plumbing are usable and testable on their own.
    """
    ctx = build_ctx_or_die(args)
    rev = getattr(args, "rev", None)
    to_spec = getattr(args, "to", None)
    if rev is not None:
        old_rev, new_rev = parse_rev_spec(rev)
        old = git_snapshot(ctx, old_rev)
        new = git_snapshot(ctx, new_rev) if new_rev else resolve_source(to_spec, ctx)
    elif (from_spec := getattr(args, "from", None)) is not None:
        old = resolve_source(from_spec, ctx)
        new = resolve_source(to_spec, ctx)
    else:
        old = git_snapshot(ctx, "HEAD")
        new = resolve_source(to_spec, ctx)
    d = diff_snapshots(ctx.cfg, old, new)
    print(f"{old.label} → {new.label}")
    if not d.changes:
        print("no changes")
        return
    for c in d.changes:
        row = c.new or c.old
        sigil = {"added": "+", "removed": "-"}.get(c.kind, "~")
        detail = {"added": "new", "removed": "removed"}.get(c.kind) or change_summary(c)
        print(f"{sigil} #{c.id} {detail} — {row.title}")


def cmd_which(args) -> None:
    """Map issue file paths (args, or stdin when none) back to issues. Each path's
    basename must be a well-formed issue filename (NNN-slug.md, matched by FILENAME_RE);
    the leading id is looked up in the index. Unknown/non-issue paths are skipped with a
    stderr note. Prints matched rows in `list` format, or bare ids with --ids; id-sorted,
    deduped."""
    ctx = build_ctx_or_die(args)
    rows = load_index(ctx)
    by_id = {r.id: r for r in rows}

    paths = list(getattr(args, "paths", None) or [])
    if not paths:
        paths = [ln.strip() for ln in sys.stdin.read().splitlines() if ln.strip()]

    seen, picked = set(), []
    for p in paths:
        name = Path(p).name
        m = FILENAME_RE.match(name)
        if not m:
            print(f"warning: not an issue path: {p}", file=sys.stderr)
            continue
        iid = file_id(m)
        if iid not in by_id:
            print(f"warning: no issue #{iid} (from {p})", file=sys.stderr)
            continue
        if iid in seen:
            continue
        seen.add(iid)
        picked.append(by_id[iid])

    picked.sort(key=get_id)
    if getattr(args, "ids", False):
        for r in picked:
            print(r.id)
    else:
        print_rows(ctx, picked)


def cmd_ready(args) -> None:
    ctx = build_ctx_or_die(args)
    g = load_graph(ctx)                                    # not-terminal leaf, every dep terminal
    # Ranked by demand, not by the declared priority alone: a medium task standing
    # between us and an urgent one outranks a high one that blocks nothing. The
    # negated `demand_vector` is compared slot by slot (see `Graph.demand_vector`),
    # then the long-standing `-points`, `id` tie-breaks. With no dependencies and no
    # parents every cone is a singleton, which is the declared-priority sort exactly.
    rows = sorted((r for r in g.rows if g.is_ready(r)),
                  key=lambda r: (*(-n for n in g.demand_vector(r)), -r.points, r.id))
    root = getattr(args, "id", None)
    if root is not None:
        # Scope by filtering the *result*, never by restricting the graph readiness is
        # computed over: blocking is effective, so a leaf here may be waiting on an
        # issue outside this subtree — commonly one authored on an ancestor. Narrow
        # the graph and those blockers vanish, making blocked work look actionable.
        kept = {n.id for n in g.subtree(resolve_ref(g.rows, root))}
        rows = [r for r in rows if r.id in kept]
    if getattr(args, "next", False):
        rows = rows[:1]
    abbrev = unique_prefix_lens([r.id for r in g.rows])
    print_rows(ctx, rows, abbrev=abbrev,
               annotate=lambda r: demand_annotation(g, r, abbrev))


def cmd_next(args) -> None:
    cmd_ready(ns_like(args, next=True))


def _print_deps_graph(ctx: Ctx, g: Graph, root_id, full: bool = False,
                      up: bool = True, down: bool = True,
                      omit_done: bool = False,
                      include_done_chains: bool = False,
                      fanout: bool = False) -> None:
    """Render the dependency DAG as a lazygit-style gutter, topologically sorted so a
    blocker always sits above what it blocks. Containment edges are drawn alongside
    authored ones, so a parent sits below the work it contains. With no id, every
    component that carries at least one authored edge (see `deps_overview_ids`),
    components separated; with an id, only that issue's directed dependency line — its
    transitive prerequisites (`up`) and dependents (`down`); pass one of `up`/`down` to
    scope to a single cone. `full` widens an id's view to its whole weakly-connected
    component (cousins included) and ignores the cone gating."""
    abbrev = unique_prefix_lens([r.id for r in g.rows])
    if root_id is not None:
        root = g.row(root_id)
        if not g.drawn_deps_of(root) and not g.drawn_dependents_of(root):
            if omit_done and g.is_terminal(root):
                return
            print(node_label(ctx, root, focal=True, abbrev=abbrev) + "  (no dependencies)")
            return
        # --full means the focal node's whole weakly-connected component, computed
        # over every issue — not over the overview set, which drops the components
        # the bare view suppresses and could therefore lose the focal node itself.
        ids = (next(c for c in graph_components(g, [r.id for r in g.rows])
                    if root_id in set(c))
               if full else g.dependency_line(root, up=up, down=down))
    else:
        ids = deps_overview_ids(g)
        if not ids:
            print("no dependencies recorded yet")
            return
    ids = filter_deps_graph_ids(g, ids, omit_done=omit_done,
                                include_done_chains=include_done_chains,
                                hide_done_chains=root_id is None)
    rows = render_graph(g, ids, fanout=fanout)
    width = max((len(gut) for r in rows if r for (_i, gut, _o) in [r]), default=0)
    # When focused on one issue, mark its row with a left-margin caret (a blank
    # 2-col gutter on every other row keeps the graph aligned); skipped for the
    # whole-graph view, which has no focal node.
    for r in rows:
        if r is None:
            print()
            continue
        iid, gutter, owners = r
        focal = iid == root_id
        marker = "" if root_id is None else (paint("▸", "bold") + " " if focal else "  ")
        painted = "".join(paint_lane(ch, ow) for ch, ow in zip(gutter, owners))
        print(f"{marker}{painted}{' ' * (width - len(gutter))}  "
              f"{node_label(ctx, g.row(iid), focal=focal, abbrev=abbrev)}")


def cmd_deps(args) -> None:
    ctx = build_ctx_or_die(args)
    g = load_graph(ctx)
    root_id = getattr(args, "id", None)
    if root_id is not None:
        root_id = resolve_ref(g.rows, root_id).id  # accept prefix / legacy-id, like every other id arg
    requires, blocks = getattr(args, "requires", False), getattr(args, "blocks", False)
    if (requires or blocks) and root_id is None:
        die("deps: --requires/--blocks scope one issue's graph — pass an issue id")
    # default (neither flag) shows both cones; one flag scopes to that direction.
    up = requires or not blocks
    down = blocks or not requires
    _print_deps_graph(ctx, g, root_id, full=getattr(args, "full", False),
                      up=up, down=down,
                      omit_done=getattr(args, "omit_done", False),
                      include_done_chains=getattr(args, "include_done_chains", False),
                      fanout=getattr(args, "fanout", False))


