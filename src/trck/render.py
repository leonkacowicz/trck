from __future__ import annotations
import os
import sys
from .config import status_role
from .graph import Graph, transitive_reduction
from .index import Ctx, Issue

# --------------------------------------------------------------------------- #
# tree rendering helpers (used by SUMMARY, tree, deps)
# --------------------------------------------------------------------------- #
STATUS_ICON = {"terminal": "●", "initial": "○", "active": "◐", None: "◐"}  # single-width, aligned


def status_icon(ctx: Ctx, name: str) -> str:
    return STATUS_ICON.get(status_role(ctx.cfg, name), "⏳")


# colour: TTY-gated, honors NO_COLOR. Never used for SUMMARY.md (a persisted file).
_ANSI = {"reset": "\033[0m", "bold": "\033[1m", "dim": "\033[2m",
         "red": "\033[31m", "green": "\033[32m", "yellow": "\033[33m",
         "blue": "\033[34m", "magenta": "\033[35m", "cyan": "\033[36m",
         "bgreen": "\033[92m", "byellow": "\033[93m", "bblue": "\033[94m",
         "bmagenta": "\033[95m", "bcyan": "\033[96m"}

# Rotating palette used to colour graph lanes; each lane keeps one colour for its
# whole descent so it can be traced through crossings (deps).
_LANE_PALETTE = ("red", "green", "yellow", "blue", "magenta", "cyan",
                 "bgreen", "byellow", "bblue", "bmagenta", "bcyan")


def _use_color() -> bool:
    if "NO_COLOR" in os.environ:  # set (any value, incl. empty) disables — no-color.org
        return False
    fc = os.environ.get("FORCE_COLOR")
    if fc is not None and fc != "0":
        return True
    return sys.stdout.isatty()


def paint(text, *codes) -> str:
    if not codes or not _use_color():
        return str(text)
    return "".join(_ANSI[c] for c in codes) + str(text) + _ANSI["reset"]


def hl_id(iid: str, abbrev=None, hash_prefix: bool = True) -> str:
    """Render an issue id with its shortest-unique-prefix bolded and the remainder
    dimmed (git-short-hash style), per the `abbrev` length map from
    `unique_prefix_lens`. With no abbrev entry, the whole id is bolded. `hash_prefix`
    prepends '#' (the row/graph form); pass False for a bare id (e.g. `show`'s field)."""
    head = "#" if hash_prefix else ""
    if abbrev and iid in abbrev:
        cut = abbrev[iid]
        rest = iid[cut:]
        return head + paint(iid[:cut], "bold") + (paint(rest, "dim") if rest else "")
    return paint(f"{head}{iid}", "bold")


def paint_lane(text, owner) -> str:
    """Colour one graph cell by its lane owner: an id picks a palette colour, the
    "node" sentinel bolds the bullet, None leaves it uncoloured. An owner may also be
    an `(id, kind)` pair; an inferred containment edge ("child") is dimmed on top of
    its palette colour, so it stays traceable through crossings while reading as
    derived rather than authored. Box-drawing has no dashed corners, so weight —
    not glyph shape — is what can carry the distinction."""
    if owner is None:
        return text
    if owner == "node":
        return paint(text, "bold")
    kind = "dep"
    if isinstance(owner, tuple):
        owner, kind = owner
    idx = int(owner) if owner.isdigit() else int.from_bytes(owner.encode(), "big")
    codes = (_LANE_PALETTE[idx % len(_LANE_PALETTE)],)
    return paint(text, *(("dim",) + codes if kind == "child" else codes))


def parse_status_filter(spec: str | None) -> tuple[set, set]:
    """Split a `--status` value into (keep, drop) sets. Comma-separates alternatives;
    a leading `!` negates a token. `ongoing,backlog` -> keep both; `!done` -> drop done."""
    keep, drop = set(), set()
    for tok in (spec or "").split(","):
        tok = tok.strip()
        if not tok:
            continue
        (drop if tok.startswith("!") else keep).add(tok.lstrip("!"))
    return keep, drop


def priority_rank(cfg: dict, prio: str) -> int:
    """Sort key: 0 = highest configured priority. Unknown priorities sort last."""
    order = cfg.get("priorities") or []
    try:
        return order.index(prio)
    except ValueError:
        return len(order)


def priority_codes(cfg: dict, prio: str) -> tuple:
    order = cfg.get("priorities") or []
    if order and prio == order[0]:
        return ("red",)
    if order and prio == order[-1]:
        return ("dim",)
    return ()


def status_codes(cfg: dict, name: str) -> tuple:
    role = status_role(cfg, name)
    if role == "terminal":
        return ("green",)
    if role == "initial":
        return ("dim",)
    return ("yellow",)


def label_tag(r: Issue) -> str:
    labels = r.labels or []
    return " [" + " ".join(labels) + "]" if labels else ""


def pr_tag(r: Issue) -> str:
    """A markdown ` · [PR](url)` suffix for SUMMARY.md rows; empty without a PR."""
    return f" · [PR]({r.pr})" if r.pr else ""


def block_annotations(g: Graph, r: Issue, on_screen=frozenset()) -> str:
    """A dim ` needs #… blocks #…` suffix exposing the blocking graph in row form.

    `needs` lists the non-terminal dependencies that make `is_blocked` true — the row's
    own *and* those lifted from an ancestor, since an authored edge is inherited by the
    author's whole subtree. A done blocker drops off (the block is cleared). An
    inherited one is tagged `(via #author)`, so the note never implies the edge was
    authored on this row — that tells you where `dep --remove` goes. It is spelled out
    only when no row between this one and the author is itself being printed
    (`on_screen` = the ids of the rows in this listing): where such a row exists it
    already carries the note, and restating it down every child is noise.

    `blocks` lists the issues that authored a dependency on this row, shown only while
    the row is itself non-terminal (a done task blocks nothing). It deliberately stays
    at the authored altitude rather than mirroring the lifting: those dependents' whole
    subtrees inherit the wait, and are exactly the rows whose `needs` reads `(via #…)`."""
    spine = g.ancestors_of(r)                          # nearest first

    def carried_above(author):
        """Does a printed row between `r` and `author` (inclusive) already say it?"""
        for a in spine:
            if a.id in on_screen:
                return True
            if a.id == author.id:
                break
        return False

    parts, needs = [], []
    for b, author in g.lifted_dep_sources(r):
        if g.is_terminal(b):
            continue
        if author.id == r.id:
            needs.append(f"#{b.id}")
        elif not carried_above(author):
            needs.append(f"#{b.id} (via #{author.id})")
    if needs:
        parts.append("needs " + " ".join(needs))
    if not g.is_terminal(r):
        blocks = [d.id for d in g.dependents_of(r)]
        if blocks:
            parts.append("blocks " + " ".join(f"#{i}" for i in blocks))
    return paint(" " + "  ".join(parts), "dim") if parts else ""


def node_label(ctx: Ctx, r: Issue, focal: bool = False, abbrev=None) -> str:
    tag = " ·epic·" if r.kind == "epic" else ""
    icon = paint(status_icon(ctx, r.status), *status_codes(ctx.cfg, r.status))
    labels = paint(label_tag(r), "dim") if r.labels else ""  # dim, as in print_rows
    emph = ("bold",) if focal else ()                        # focal row of `deps NNN`
    # id always gets the shortest-unique-prefix highlight (as in `list`); the focal
    # node is set apart by the ▸ caret + bold title, not a wholly-bold id.
    return f"{hl_id(r.id, abbrev)} {icon} {paint(r.title, *emph)}{tag}{labels}"


# --- depends_on DAG rendering (deps): lazygit-style one-row-per-node ---
# A node sits in a lane; lanes[c] holds the dependent a lane is routing to. Every
# merge/fork is co-located on its node's own row as box-drawing corners + horizontal
# runs (no blank edge rows). Each cell maps its connection set {U,D,L,R} to a glyph.
_GRAPH_GLYPH = {
    frozenset("UD"): "│", frozenset("LR"): "─",
    frozenset("UR"): "╰", frozenset("UL"): "╯",
    frozenset("DR"): "╭", frozenset("DL"): "╮",
    frozenset("UDR"): "├", frozenset("UDL"): "┤",
    frozenset("ULR"): "┴", frozenset("DLR"): "┬",
    frozenset("UDLR"): "┼",
    frozenset("U"): "│", frozenset("D"): "│",
    frozenset("L"): "─", frozenset("R"): "─",
}


def drawn_edges(g: Graph, ids, hier: bool = True, reduce: bool = True,
                fanout: bool = False) -> dict:
    """The drawn edge set restricted to `ids`, as `{source: [(target, kind), …]}`.
    With `reduce`, transitively reduced (see `transitive_reduction`).

    Callers pass the ids they are *about to draw* — already done-filtered — which is
    what keeps the reduction honest: an edge may only be dropped in favour of a path
    that is itself on screen.

    An inherited edge is dropped when an ancestor between this node and the issue
    that authored it is on screen: that row already carries the dependency, and the
    containment edges connect the two. Inheritance is uniform by construction — an
    authored edge reaches *every* descendant — so restating it under each child would
    replace one parent-altitude edge with a fan of n, and reduction would then delete
    the parent's own edge as implied by its children. Suppressing the fan up front is
    what keeps a dependency at the altitude it was authored. This mirrors how the
    `needs #X (via #P)` row note picks its moment to speak. `fanout` keeps the fan."""
    idset = set(ids)

    def carried_above(r, author):
        """Is a drawn row between `r` and `author` (inclusive) already saying it?"""
        for a in g.ancestors_of(r):
            if a.id in idset:
                return True
            if a.id == author.id:
                break
        return False

    edges = {}
    for i in idset:
        r = g.by_id[i]
        hidden = set() if fanout or not hier else {
            b.id for b, author in g.inherited_deps_of(r) if carried_above(r, author)}
        edges[i] = [(d.id, k) for d, k in g.drawn_deps_of(r, hier)
                    if d.id in idset and not (k == "inherited" and d.id in hidden)]
    return transitive_reduction(edges) if reduce else edges


def graph_components(g: Graph, ids, hier: bool = True, edges: dict | None = None) -> list[list[int]]:
    """Weakly-connected components over the drawn edges restricted to `ids` (edges
    undirected; containment included unless `hier` is off). Each component is
    id-sorted; components order by smallest member — so a node's cluster renders as
    one contiguous, separable block. Pass `edges` to group over an already-built
    (e.g. reduced) edge set rather than recomputing it."""
    idset = set(ids)
    if edges is None:
        edges = drawn_edges(g, idset, hier, reduce=False)
    adj: dict[int, set] = {i: set() for i in idset}
    for i in idset:
        for dep, _kind in edges.get(i, ()):
            if dep in idset:
                adj[i].add(dep); adj[dep].add(i)
    seen, comps = set(), []
    for start in sorted(idset):
        if start in seen:
            continue
        stack, comp = [start], []
        seen.add(start)
        while stack:
            x = stack.pop(); comp.append(x)
            for y in adj[x]:
                if y not in seen:
                    seen.add(y); stack.append(y)
        comps.append(sorted(comp))
    return sorted(comps, key=min)


def _graph_topo(g: Graph, ids, hier: bool = True,
                edges: dict | None = None) -> tuple[list[int], dict[int, list[int]], dict]:
    """Topological order over the drawn edges within `ids`, prerequisites first. Returns
    (order, dependents) where dependents[i] is i's id-sorted in-set dependents — the
    lanes node i opens downward.

    Tie-break is DFS by locality: among nodes that are ready, take the one unblocked
    MOST recently (a dependent of the node just placed) so a branch is drawn to its
    end before the next one starts — its lane closes on the next row instead of
    lingering open beside a parallel branch. This keeps bullets in a single column
    per branch (fewer crossings, shorter edges) versus an id-priority queue, which
    interleaves branches breadth-first. Siblings unblocked together are visited in
    ascending id order, so the layout stays fully deterministic."""
    idset = set(ids)
    if edges is None:
        edges = drawn_edges(g, idset, hier, reduce=False)
    requires = {i: [d for d, _k in edges.get(i, ()) if d in idset] for i in idset}
    kinds = {(i, d): k for i in idset for d, k in edges.get(i, ()) if d in idset}
    dependents: dict[int, list[int]] = {i: [] for i in idset}
    for i in idset:
        for d in requires[i]:
            dependents[d].append(i)
    for i in idset:
        dependents[i].sort()
    indeg = {i: len(requires[i]) for i in idset}
    # LIFO stack = depth-first; push newly-ready nodes high-id-first so the lowest
    # id is on top and popped next (ascending order within a freshly-unblocked set).
    stack = sorted((i for i in idset if indeg[i] == 0), reverse=True)
    order = []
    while stack:
        n = stack.pop()
        order.append(n)
        newly = []
        for dep in dependents[n]:
            indeg[dep] -= 1
            if indeg[dep] == 0:
                newly.append(dep)
        stack.extend(sorted(newly, reverse=True))
    return order, dependents, kinds


def _graph_component_rows(g: Graph, comp, hier: bool = True,
                          edges: dict | None = None) -> list[tuple[int, str, list]]:
    """Render one connected component, one row per node. Returns (id, gutter, owners):
    gutter is the plain box-drawing string; owners is a per-character lane owner (an
    `(id, kind)` pair, the "node" sentinel for the bullet, or None) for colouring.

    A lane is opened at its prerequisite's row and closed at the row of the dependent
    it heads to, so each lane column carries exactly one edge — which is why the edge
    kind rides along with the lane and an inferred containment edge can be drawn
    differently from an authored one."""
    order, dependents, kinds = _graph_topo(g, comp, hier, edges)
    lanes: list = []                         # lanes[c] = (dependent id, edge kind)
    rows = []
    for n in order:
        top = list(lanes)
        arriving = [c for c, t in enumerate(top) if t is not None and t[0] == n]
        pos = arriving[0] if arriving else (top.index(None) if None in top else len(top))
        bottom = list(top)
        while len(bottom) <= pos:
            bottom.append(None)
        for c in arriving:
            bottom[c] = None
        bottom[pos] = None
        started = []
        for k, d in enumerate(dependents[n]):
            lane = (d, kinds[(d, n)])        # the lane carries the edge d -> n
            if k == 0:
                bottom[pos] = lane; started.append(pos)
            else:
                # Reuse the free column NEAREST the node, not the leftmost gap:
                # same lane count (still greedy interval colouring) but a shorter
                # horizontal bridge and fewer crossings. Ties go to the lower column.
                free = [c for c, t in enumerate(bottom) if t is None]
                c = min(free, key=lambda c: (abs(c - pos), c)) if free else len(bottom)
                if c == len(bottom):
                    bottom.append(None)
                bottom[c] = lane; started.append(c)

        width = max(len(top), len(bottom), pos + 1)
        dirs = [set() for _ in range(width)]
        owner: list = [None] * width
        opri = [-1] * width

        def colour(c, who, pri):             # higher priority wins (vertical > bridge)
            if who is not None and pri > opri[c]:
                opri[c] = pri; owner[c] = who

        def bridge(a, b, who):               # horizontal run between columns a and b
            for k in range(min(a, b) + 1, max(a, b)):
                dirs[k] |= {"L", "R"}; colour(k, who, 1)

        for c in range(width):               # continuing lanes pass straight through
            if c < len(top) and top[c] is not None and c not in arriving and c != pos:
                dirs[c] |= {"U", "D"}; colour(c, top[c], 2)
        for a in arriving:                   # lanes merging up into the node
            dirs[a].add("U")
            if a == pos:
                continue
            lane = top[a]                    # (n, kind of the edge that opened it)
            dirs[a].add("R" if a < pos else "L"); colour(a, lane, 2)
            dirs[pos].add("L" if a < pos else "R"); colour(pos, lane, 1)
            bridge(a, pos, lane)
        for b in started:                    # lanes forking down out of the node
            d = bottom[b]
            dirs[b].add("D")
            if b == pos:
                continue
            dirs[b].add("L" if b > pos else "R"); colour(b, d, 2)
            dirs[pos].add("R" if b > pos else "L"); colour(pos, d, 1)
            bridge(pos, b, d)

        chars, owners = [], []
        for c in range(width):
            if c == pos:
                chars.append("●"); owners.append("node")
            else:
                chars.append(_GRAPH_GLYPH.get(frozenset(dirs[c]), " ")); owners.append(owner[c])
            chars.append("─" if "R" in dirs[c] else " "); owners.append(owner[c])
        while chars and chars[-1] == " ":    # trim trailing blanks, keep owners aligned
            chars.pop(); owners.pop()
        rows.append((n, "".join(chars), owners))
        while bottom and bottom[-1] is None:
            bottom.pop()
        lanes = bottom
    return rows


def render_graph(g: Graph, ids, hier: bool = True, reduce: bool = True,
                 fanout: bool = False) -> list:
    """Render the dependency DAG over `ids` as lazygit-style rows, grouped by
    weakly-connected component with a `None` separator between groups. Each
    non-separator row is (id, gutter, owners).

    The edge set is built and transitively reduced here, over exactly the ids being
    drawn. Doing it at this point rather than at the caller is what makes the
    ordering trap impossible: `ids` has already been done-filtered, so an edge can
    never be dropped in favour of a path through a node that isn't rendered — which
    would leave its endpoints looking unrelated."""
    edges = drawn_edges(g, ids, hier, reduce, fanout)
    rows = []
    for comp in graph_components(g, ids, hier, edges):
        if rows:
            rows.append(None)
        rows.extend(_graph_component_rows(g, comp, hier, edges))
    return rows


def deps_overview_ids(g: Graph) -> set:
    """The id set for the bare `deps` view: every weakly-connected component holding
    at least one *authored* edge, taken whole.

    Containment edges connect nearly the whole forest, so the old rule — "every issue
    touching an edge" — would now match almost every issue and turn this view into
    `list`. Selecting by authored edges keeps it about ordering constraints, and a
    family joins the view as soon as anything in it is actually ordered.

    Components are kept or dropped *whole* rather than filtering node by node: a
    parent shown without some of its children would misreport what it is waiting on,
    which is precisely the question this view exists to answer."""
    ids = [r.id for r in g.rows]
    keep = set()
    for comp in graph_components(g, ids):
        members = set(comp)
        if any(d in members for i in comp for d in g.by_id[i].depends_on):
            keep |= members
    return keep


def filter_deps_graph_ids(g: Graph, ids, *, omit_done: bool = False,
                          include_done_chains: bool = False,
                          hide_done_chains: bool = True) -> set[int]:
    """Apply display-only done filtering to a deps graph id set.

    Fully terminal components are hidden only for the whole-graph view. Removing
    done nodes happens by shrinking the id set; render_graph then recomputes
    components over the remaining open-only subgraph, so no synthetic edges are
    introduced across omitted terminal nodes.
    """
    kept = set(ids)
    if hide_done_chains and not include_done_chains:
        for comp in graph_components(g, kept):
            if all(g.is_terminal(g.row(i)) for i in comp):
                kept.difference_update(comp)
    if omit_done:
        kept = {i for i in kept if not g.is_terminal(g.row(i))}
    return kept


