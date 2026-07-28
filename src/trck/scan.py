from __future__ import annotations
from .config import check_kind, check_points, check_pr, check_priority, check_resolution, check_status_flags, check_status_roles, reconcile, status_names
from .constants import FIELD_KEY_RE, FILENAME_RE, SLUG_RE, die
from .graph import Graph
from .index import Ctx, DEFAULT_POINTS, Issue, file_id, filename, load_index

# --------------------------------------------------------------------------- #
# filesystem scan + validation
# --------------------------------------------------------------------------- #
def scan_files(ctx: Ctx) -> dict:
    """Map id -> (status_folder, slug, filename) for every issue markdown on disk."""
    found = {}
    for name in status_names(ctx.cfg):
        d = ctx.dir / name
        if not d.is_dir():
            continue
        for p in sorted(d.glob("*.md")):
            m = FILENAME_RE.match(p.name)
            if not m:
                continue
            iid = file_id(m)
            if iid in found:
                die(f"duplicate issue id {iid} on disk: {found[iid][0]} and {name}")
            found[iid] = (name, m.group(2), p.name)
    return found


def validate(ctx: Ctx, rows: list[Issue] | None = None) -> tuple[list[str], list[str]]:
    """Validate the index against the on-disk files. Callers that already hold the
    current rows (e.g. `finalize` right after writing them) may pass them to skip a
    redundant re-parse of index.jsonl; the file scan still reads the folders, so the
    filesystem-vs-index consistency check is unaffected. Omit `rows` to validate the
    persisted index as loaded from disk."""
    errors, warnings = [], []
    errors.extend(check_status_roles(ctx.cfg))
    errors.extend(check_status_flags(ctx.cfg))
    if rows is None:
        rows = load_index(ctx)
    files = scan_files(ctx)
    g = Graph(ctx.cfg, rows)
    by_id = g.by_id
    names = set(status_names(ctx.cfg))

    for iid, r in by_id.items():
        if iid not in files:
            errors.append(f"#{iid} in index but no markdown file on disk")
            continue
        folder, slug, fname = files[iid]
        if r.status != folder:
            errors.append(f"#{iid} index status '{r.status}' != folder '{folder}'")
        if r.slug != slug:
            errors.append(f"#{iid} index slug '{r.slug}' != filename slug '{slug}'")
        if fname != filename(r):
            errors.append(f"#{iid} filename '{fname}' != expected '{filename(r)}'")
        if not r.slug or not SLUG_RE.match(r.slug):
            errors.append(f"#{iid} bad slug '{r.slug}'")
        if r.status not in names:
            errors.append(f"#{iid} unknown status '{r.status}'")
        if (m := check_kind(ctx.cfg, r.kind)):
            errors.append(f"#{iid} {m}")
        if (m := check_priority(ctx.cfg, r.priority)):
            errors.append(f"#{iid} {m}")
        pts = r.points  # parse guarantees an int; here we check the value/placement
        if not g.is_leaf(r):
            if pts != DEFAULT_POINTS:
                errors.append(f"#{iid} has children but carries points {pts!r} "
                              f"(derived from leaves, must be unset)")
        elif (m := check_points(pts)):
            errors.append(f"#{iid} {m}")
        if r.resolution is not None and (m := check_resolution(ctx.cfg, r.resolution)):
            errors.append(f"#{iid} {m}")
        if r.pr is not None and (m := check_pr(r.pr)):
            errors.append(f"#{iid} {m}")
        for k, v in r.extra.items():
            if not FIELD_KEY_RE.match(k):
                errors.append(f"#{iid} bad custom field key '{k}'")
            elif not isinstance(v, str):
                errors.append(f"#{iid} custom field '{k}' must be a string, got {v!r}")
    for iid in files:
        if iid not in by_id:
            errors.append(f"#{iid} markdown file on disk but no index row")

    for r in rows:
        if r.parent is not None and r.parent not in by_id:
            errors.append(f"#{r.id} parent #{r.parent} does not exist")
        for dep in r.depends_on:
            if dep not in by_id:
                errors.append(f"#{r.id} depends_on #{dep} which does not exist")

    for cyc in g.parent_cycles():  # one error per cycle, not one per node
        chain = " -> ".join(f"#{c}" for c in (*cyc, cyc[0]))
        errors.append(f"parent cycle: {chain}")

    # Effective (lifted) dependency cycles — a superset of the authored ones. This
    # surfaces inherited deadlocks that arrived via hand-edit / import / `mv`; the
    # message names the authored edges + parent links behind the implied loop.
    for cyc in g.effective_cycles():  # one error per cycle
        errors.append(f"effective dependency cycle: {g.describe_effective_cycle(cyc)}")

    # A non-overridden parent's status must equal the rollup of its children (#67);
    # normalize_statuses maintains this after every verb, so a violation here means a
    # hand-edited index (or a `manual_status` opt-out, which is exempt).
    for parent_row in rows:
        kids = g.children_of(parent_row)
        if not kids or parent_row.manual_status:
            continue
        desired = reconcile(ctx.cfg, [k.status for k in kids])
        if desired and parent_row.status != desired:
            errors.append(
                f"#{parent_row.id} status '{parent_row.status}' should be "
                f"'{desired}' (derived from its children; pin it with a manual `mv` "
                f"to override)"
            )
    for r in rows:
        if g.is_terminal(r):
            for dep in r.depends_on:
                if dep in by_id and not g.is_terminal(by_id[dep]):
                    warnings.append(f"#{r.id} is terminal but depends on non-terminal #{dep}")
    return errors, warnings
