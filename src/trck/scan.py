from __future__ import annotations
from .config import check_points, check_priority, check_resolution, check_review_url, check_vestigial_vocabulary, reconcile, status_names
from .constants import FIELD_KEY_RE, FILENAME_RE, ITEMS_DIR, SLUG_RE, die
from .graph import Graph
from .index import Ctx, DEFAULT_POINTS, Issue, file_id, filename, load_index

# --------------------------------------------------------------------------- #
# filesystem scan + validation
# --------------------------------------------------------------------------- #
def scan_files(ctx: Ctx) -> dict:
    """Map id -> (slug, filename) for every issue markdown in the items dir. Status
    is not encoded in the path, so the folder component the old layout returned is
    gone; two files can still claim one id via different slugs, which is fatal."""
    found = {}
    d = ctx.dir / ITEMS_DIR
    if not d.is_dir():
        return found
    for p in sorted(d.glob("*.md")):
        m = FILENAME_RE.match(p.name)
        if not m:
            continue
        iid = file_id(m)
        if iid in found:
            die(f"duplicate issue id {iid} on disk: {found[iid][1]} and {p.name}")
        found[iid] = (m.group(2), p.name)
    return found


def validate(ctx: Ctx, rows: list[Issue] | None = None) -> tuple[list[str], list[str]]:
    """Validate the index against the on-disk files. Callers that already hold the
    current rows (e.g. `finalize` right after writing them) may pass them to skip a
    redundant re-parse of index.jsonl; the file scan still reads the folders, so the
    filesystem-vs-index consistency check is unaffected. Omit `rows` to validate the
    persisted index as loaded from disk."""
    errors, warnings = [], []
    warnings.extend(check_vestigial_vocabulary(ctx.cfg))
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
        slug, fname = files[iid]
        if r.slug != slug:
            errors.append(f"#{iid} index slug '{r.slug}' != filename slug '{slug}'")
        if fname != filename(r):
            errors.append(f"#{iid} filename '{fname}' != expected '{filename(r)}'")
        if not r.slug or not SLUG_RE.match(r.slug):
            errors.append(f"#{iid} bad slug '{r.slug}'")
        if r.status not in names:
            errors.append(f"#{iid} unknown status '{r.status}'")
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
        # `(status, closed, resolution)` is one unit: `move_issue` clears both dates on
        # any move to a non-terminal status, and `cmd_mv` refuses `--resolution` unless
        # the target is terminal. So a non-terminal row carrying either is a row no verb
        # can have written — a hand-edit, or a field-wise merge of index.jsonl that
        # resolved the tuple's members independently (#ey2aruc). Two separate errors: a
        # merge can produce either one alone. `review_url` is deliberately not in this set — a
        # closed issue keeping its pull-request link is the review record for the change
        # that resolved it, and an issue in flight linking one is what `review` does.
        if not g.is_terminal(r):
            if r.resolution is not None:
                errors.append(f"#{iid} is '{r.status}' (not terminal) but carries "
                              f"resolution '{r.resolution}'")
            if r.closed is not None:
                errors.append(f"#{iid} is '{r.status}' (not terminal) but carries "
                              f"closed '{r.closed}'")
        if r.review_url is not None and (m := check_review_url(r.review_url)):
            errors.append(f"#{iid} {m}")
        # Sorted, not insertion order: these are reported *after* a mutating verb has
        # already rewritten the index, and canonical form sorts the extra keys. Sorting
        # here makes the diagnostics run in the same order as the file the reader is about
        # to open, and makes them independent of how the row happened to be typed.
        for k, v in sorted(r.extra.items()):
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
