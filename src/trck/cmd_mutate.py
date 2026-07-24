from __future__ import annotations
import re
import shutil
from .config import check_kind, check_points, check_priority, check_resolution, default_priority, initial_status, is_terminal, reconcile
from .constants import SLUG_RE, die, now_utc, slugify
from .finalize import finalize
from .graph import Graph, gen_id
from .index import DEFAULT_POINTS, Issue, build_ctx_or_die, check_field_key, get_row, issue_path, load_index, resolve_ref
from .templates import TEMPLATE, guard_dep_edge, guard_effective_acyclic, move_issue, parse_ids

def cmd_new(args) -> None:
    ctx = build_ctx_or_die(args)
    rows = load_index(ctx)
    iid = gen_id(ctx)
    slug = args.slug or slugify(args.title)
    if not SLUG_RE.match(slug):
        die(f"computed slug '{slug}' is invalid; pass --slug")
    priority = args.priority or default_priority(ctx.cfg)
    if (m := check_priority(ctx.cfg, priority)):
        die(m)
    rawpoints = getattr(args, "points", None)
    points = DEFAULT_POINTS if rawpoints is None else rawpoints
    if (m := check_points(points)):
        die(m)
    kind = args.kind or ctx.cfg["kinds"][0]
    if (m := check_kind(ctx.cfg, kind)):
        die(m)
    parent = None if args.parent is None else resolve_ref(rows, args.parent).id
    deps = [resolve_ref(rows, tok).id for tok in parse_ids(args.depends)]
    row = Issue(
        id=iid, slug=slug, title=args.title, kind=kind,
        status=initial_status(ctx.cfg), priority=priority, points=points,
        parent=parent, depends_on=deps, spec=args.spec, created=now_utc(),
    )
    path = issue_path(ctx, row)
    if path.exists():
        die(f"{path} already exists")
    rows.append(row)
    # Guard the new node's dependency edges against the candidate graph (its parent
    # is already set, so an inherited cousin cycle is caught) before writing anything.
    g = Graph(ctx.cfg, rows)
    for dep in row.depends_on:
        guard_dep_edge(g, row.id, dep)
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(TEMPLATE.format(title=args.title))
    finalize(ctx, rows)
    print(path)


def cmd_mv(args) -> None:
    ctx = build_ctx_or_die(args)
    rows = load_index(ctx)
    row = get_row(rows, args.id)
    resolution = getattr(args, "resolution", None)
    if resolution is not None:
        if not is_terminal(ctx.cfg, args.status):
            die("--resolution is only valid when moving to a terminal status")
        if (m := check_resolution(ctx.cfg, resolution)):
            die(m)
    move_issue(ctx, row, args.status)
    if resolution is not None:
        row.resolution = resolution
    # Moving a node that has children is an override of the rollup (#67) — but only
    # when the requested status differs from what derivation would produce. A move
    # that agrees with the children leaves the node unpinned (nothing to override),
    # which is what lets #18's `--recurse` compose without a special case.
    kids = Graph(ctx.cfg, rows).children_of(row)
    if kids:
        row.manual_status = (row.status != reconcile(ctx.cfg, [k.status for k in kids]))
    finalize(ctx, rows)
    print(issue_path(ctx, row))


def cmd_set(args) -> None:
    ctx = build_ctx_or_die(args)
    rows = load_index(ctx)
    g = Graph(ctx.cfg, rows)
    row = get_row(rows, args.id)
    if getattr(args, "auto", False):
        row.manual_status = False  # return to derivation; finalize re-derives + cascades
    if args.priority:
        if (m := check_priority(ctx.cfg, args.priority)):
            die(m)
        row.priority = args.priority
    if getattr(args, "points", None) is not None:
        if (m := check_points(args.points)):
            die(m)
        if not g.is_leaf(row):
            die(f"#{row.id} has children; points is derived from them, not set")
        row.points = args.points
    if args.parent is not None:
        if args.parent == "none":
            row.parent = None
        else:
            row.parent = resolve_ref(rows, args.parent).id
    if args.spec is not None:
        row.spec = None if args.spec == "none" else args.spec
    if args.kind:
        if (m := check_kind(ctx.cfg, args.kind)):
            die(m)
        row.kind = args.kind
    for spec in (getattr(args, "field", None) or []):
        if "=" not in spec:
            die(f"--field expects key=value, got '{spec}'")
        key, val = spec.split("=", 1)
        if (m := check_field_key(key)):
            die(m)
        if val == "":
            row.extra.pop(key, None)  # empty value clears (alias for --unset)
        else:
            row.extra[key] = val
    for key in (getattr(args, "unset", None) or []):
        if (m := check_field_key(key)):
            die(m)
        row.extra.pop(key, None)
    # Re-parenting changes the dependency lifting, so a reparent can introduce an
    # effective cycle. Guard the candidate state before persisting (Option B).
    if args.parent is not None:
        guard_effective_acyclic(ctx, rows)
    old = issue_path(ctx, row)
    if args.slug:
        if not SLUG_RE.match(args.slug):
            die(f"invalid slug '{args.slug}'")
        row.slug = args.slug
    if args.title:
        row.title = args.title
    new = issue_path(ctx, row)
    if old.resolve() != new.resolve():
        shutil.move(str(old), str(new))
    if args.title:
        text = new.read_text()
        text = re.sub(r"^# .*$", f"# {args.title}", text, count=1, flags=re.MULTILINE)
        new.write_text(text)
    finalize(ctx, rows)
    print(f"#{row.id} updated")


def cmd_label(args) -> None:
    ctx = build_ctx_or_die(args)
    rows = load_index(ctx)
    row = get_row(rows, args.id)
    labels = list(row.labels)
    for lab in (args.add or []):
        if lab and lab not in labels:
            labels.append(lab)
    for lab in (args.remove or []):
        if lab in labels:
            labels.remove(lab)
    row.labels = sorted(labels)
    finalize(ctx, rows)
    print(f"#{row.id} labels={row.labels}")


def cmd_dep(args) -> None:
    ctx = build_ctx_or_die(args)
    rows = load_index(ctx)
    g = Graph(ctx.cfg, rows)
    row = get_row(rows, args.id)
    add = None if args.add is None else resolve_ref(rows, args.add).id
    rem = None if args.remove is None else resolve_ref(rows, args.remove).id
    deps = list(row.depends_on)
    if add is not None:
        guard_dep_edge(g, row.id, add)
        if add not in deps:
            deps.append(add)
    if rem is not None and rem in deps:
        deps.remove(rem)
    row.depends_on = sorted(deps)
    finalize(ctx, rows)
    print(f"#{row.id} depends_on={row.depends_on}")


