from __future__ import annotations
import secrets
from .config import PRIORITIES, is_actionable, is_terminal, reconcile
from .constants import FILENAME_RE, ID_ALPHABET, ID_LEN, ITEMS_DIR
from .index import Ctx, DEFAULT_POINTS, Issue, file_id, get_id, get_row, load_index
from .render import priority_rank
from .templates import apply_status

# --------------------------------------------------------------------------- #
# issue graph (derived read view)
# --------------------------------------------------------------------------- #
class Graph:
    """A derived read view over a loaded index: id lookup, the parent/child and
    depends_on edges, and the readiness/blocking predicates every read command
    needs. Built once per command from `(cfg, rows)`; never mutated — `rows`
    stays the source of truth and no back-references are bolted onto `Issue`.

    The list accessors (`children_of`/`dependents_of`/`requires_of`) return
    id-sorted lists so callers don't re-sort."""

    def __init__(self, cfg: dict, rows: list[Issue]):
        self.cfg = cfg
        self.rows = rows
        self.by_id: dict[int, Issue] = {r.id: r for r in rows}
        self._children: dict[int, list[Issue]] = {}    # parent id -> children
        self._dependents: dict[int, list[Issue]] = {}  # dep id -> dependents
        for r in rows:
            if r.parent is not None:
                self._children.setdefault(r.parent, []).append(r)
            for d in r.depends_on:
                self._dependents.setdefault(d, []).append(r)
        self._parents = set(self._children)            # ids with >= 1 child
        self._demands: dict[str, set[str]] | None = None  # reverse blocking, built once
        self._cones: dict[str, set[str]] = {}          # per-id demand closure

    # lookup
    def get(self, issue_id: int) -> Issue | None:
        return self.by_id.get(issue_id)

    def row(self, issue_id: int) -> Issue:
        return get_row(self.rows, issue_id)            # dies if missing

    # edges (id-sorted unless a sibling sort `key` is given)
    def children_of(self, r: Issue, key=get_id) -> list[Issue]:
        return sorted(self._children.get(r.id, []), key=key)

    def dependents_of(self, r: Issue) -> list[Issue]:
        return sorted(self._dependents.get(r.id, []), key=get_id)

    def requires_of(self, r: Issue) -> list[Issue]:
        return [self.by_id[d] for d in sorted(r.depends_on) if d in self.by_id]

    # --- inferred (containment) edges: display-only ------------------------ #
    # A parent is done exactly when all its children are, which *is* a dependency:
    # `parent -> child`. Nobody authors those edges; they follow from what parenthood
    # means. Surfacing them to the renderer is what lets `deps <epic>` answer "what is
    # needed to complete this epic". They are never written back to the index — only
    # `dep --add/--remove` touches `depends_on`.
    #
    # The combined relation needs no new invariant: `_eff_reach` already composes
    # `lifted_deps` with each target's `subtree`, and `would_cycle`/`validate` reject
    # any authored edge that would close a loop in it, so authored + containment is
    # acyclic for any tracker that passes `check`.

    def inherited_deps_of(self, r: Issue) -> list[tuple[Issue, Issue]]:
        """`r`'s inherited dependencies as `(target, author)` — the lifting rule's
        source side, minus what `r` authored itself. Targets always sit outside `r`'s
        own subtree: an authored ancestor/descendant edge is rejected as a cycle, so
        an inherited one can never point at something `r` contains."""
        return [(b, a) for b, a in self.lifted_dep_sources(r) if a.id != r.id]

    def drawn_deps_of(self, r: Issue, hier: bool = True) -> list[tuple[Issue, str]]:
        """The targets `r` depends on *as drawn*, each paired with its edge kind:
        `"dep"` for an authored `depends_on`, `"child"` for an inferred containment
        edge, `"inherited"` for a dependency lifted from an ancestor. Authored first,
        id-sorted within a kind, so the layout stays deterministic. `hier=False`
        yields the authored graph untouched.

        Inherited edges are included here so a cone expanded from a child reaches the
        blocker it is actually waiting on. Whether they are *drawn* is a view
        decision, made in `drawn_edges` — under a visible parent they are pure
        fan-out."""
        out = [(self.by_id[d], "dep") for d in sorted(r.depends_on) if d in self.by_id]
        if hier:
            out += [(c, "child") for c in self.children_of(r)]
            out += [(b, "inherited") for b, _a in self.inherited_deps_of(r)]
        return out

    def drawn_dependents_of(self, r: Issue, hier: bool = True) -> list[Issue]:
        """The reverse of `drawn_deps_of`: issues that depend on `r` as drawn — its
        authored dependents plus, when `hier`, the parent that contains it."""
        out = list(self.dependents_of(r))
        if hier and r.parent is not None and r.parent in self.by_id:
            out.append(self.by_id[r.parent])
        return out

    def ancestors_of(self, r: Issue) -> list[Issue]:
        """The parent spine above `r`, nearest first, up to a root. A parent that
        points at a missing id ends the spine (that node is treated as a root); a
        parent cycle is broken by the `seen` guard so malformed data can't loop."""
        chain, seen, cur = [], {r.id}, r
        while cur.parent is not None:
            nxt = self.by_id.get(cur.parent)
            if nxt is None or nxt.id in seen:
                break
            chain.append(nxt)
            seen.add(nxt.id)
            cur = nxt
        return chain


    def match_closure(self, matches) -> tuple[set[int], set[int]]:
        """For a filtered forest: given a per-row match predicate, return
        `(shown, dim)` id sets. `shown` is every matched node plus the ancestor
        spine of each match (so a deep match keeps its parents) — equivalently,
        "show a node iff it matches or has a descendant that matches". `dim` is the
        shown nodes that did not match themselves, rendered as dimmed context."""
        matched = {r.id for r in self.rows if matches(r)}
        shown = set(matched)
        for r in self.rows:
            if r.id in matched:
                shown.update(a.id for a in self.ancestors_of(r))
        return shown, shown - matched

    def subtree(self, r: Issue) -> list[Issue]:
        """`r` plus every descendant (the containment closure below `r`). A `seen`
        guard makes it terminate on a malformed parent cycle (reported by
        `validate`). This is the target-side of the dependency lifting rule."""
        out, seen, stack = [], set(), [r]
        while stack:
            n = stack.pop()
            if n.id in seen:
                continue
            seen.add(n.id)
            out.append(n)
            stack.extend(self._children.get(n.id, []))
        return out

    def lifted_dep_sources(self, r: Issue) -> list[tuple[Issue, Issue]]:
        """`lifted_deps` with each target paired to the issue that authored the edge
        (`r` itself, or the nearest ancestor it was inherited from). Nearest author
        first, id-sorted within an author, and a target reached twice keeps its
        nearest author — so an edge `r` authored itself never reads as inherited.
        Callers that only need the targets use `lifted_deps`; row rendering needs the
        author too, to say where the edge actually lives."""
        out, seen = [], set()
        for a in (r, *self.ancestors_of(r)):
            for d in sorted(a.depends_on):
                b = self.by_id.get(d)
                if b is not None and b.id not in seen:
                    seen.add(b.id)
                    out.append((b, a))
        return out

    def lifted_deps(self, r: Issue) -> list[Issue]:
        """The authored dependency targets visible to `r` through the parent
        hierarchy: `r`'s own `depends_on` plus every ancestor's — the *source-side*
        of the lifting rule (an authored edge a->b is inherited by all of
        subtree(a)). This is the single shared lifting primitive: `is_blocked`
        reads the target statuses one-sidedly, the cycle traversal expands each
        target's subtree, and `block_annotations` renders the row's note."""
        return [b for b, _ in self.lifted_dep_sources(r)]

    # predicates
    def is_terminal(self, r: Issue) -> bool:
        return is_terminal(self.cfg, r.status)

    def is_actionable(self, r: Issue) -> bool:
        return is_actionable(self.cfg, r.status)

    def is_blocked(self, r: Issue) -> bool:
        """One-sided effective blocking: `r` is blocked iff it, or any ancestor,
        has an authored dependency on a non-terminal issue. The depended-on side
        needs no expansion — a parent's status is terminal only when its whole
        subtree is (rollup), so "wait for b" already means "wait for subtree(b)"."""
        return any(not self.is_terminal(b) for b in self.lifted_deps(r))

    def is_leaf(self, r: Issue) -> bool:
        return r.id not in self._parents

    def is_ready(self, r: Issue) -> bool:
        """An unblocked leaf you could pick up right now. A status may opt out of
        being pickable (`"actionable": false`) — an issue awaiting review is in
        flight, not available — without becoming terminal, so it still blocks."""
        return (not self.is_terminal(r) and self.is_actionable(r)
                and self.is_leaf(r) and not self.is_blocked(r))

    # --- demand: effective blocking, reversed ----------------------------- #
    # `is_blocked` asks what an issue is waiting on. Ranking `ready` needs the
    # mirror image — who is waiting on *it* — because a medium task standing
    # between us and an urgent one is worth more than a high one that blocks
    # nothing. Purely derived, like every other predicate here: nothing about
    # demand is stored, and `list --sort priority` still sorts the declared field.

    def _demand_edges(self) -> dict[str, set[str]]:
        """`id -> the ids directly waiting on it`, over non-terminal issues only.

        Two channels feed it. An authored edge `a -> b` is inherited by all of
        `subtree(a)` and satisfied only by all of `subtree(b)` (the lifting rule),
        so every member of the target subtree is demanded by every member of the
        source subtree — the same relation `is_blocked` reads, turned around. And
        a node is demanded by its parent, which is not done until its children
        are; that alone lets an urgent epic rank its own leaves.

        Terminal issues are dropped from both ends, so they neither count nor
        conduct: an urgent dependent closed as `wontfix` stops making its
        blockers urgent, exactly as it stops blocking. Built once per graph."""
        if self._demands is None:
            rev: dict[str, set[str]] = {}
            for r in self.rows:
                if self.is_terminal(r):
                    continue
                p = self.by_id.get(r.parent) if r.parent is not None else None
                if p is not None and not self.is_terminal(p):
                    rev.setdefault(r.id, set()).add(p.id)
            for a in self.rows:
                srcs = {n.id for n in self.subtree(a) if not self.is_terminal(n)}
                if not srcs:
                    continue
                for b in self.requires_of(a):
                    for t in self.subtree(b):
                        if not self.is_terminal(t):
                            rev.setdefault(t.id, set()).update(srcs)
            self._demands = rev
        return self._demands

    def demand_cone(self, r: Issue) -> set[str]:
        """`r` plus every non-terminal issue transitively waiting on it — the
        transitive closure of `_demand_edges` from `r`. `r` is always in its own
        cone, so an issue nobody waits on still ranks by its own priority."""
        cone = self._cones.get(r.id)
        if cone is None:
            rev = self._demand_edges()
            cone, stack = {r.id}, [r.id]
            while stack:
                for n in rev.get(stack.pop(), ()):
                    if n not in cone:
                        cone.add(n)
                        stack.append(n)
            self._cones[r.id] = cone
        return cone

    def demand_vector(self, r: Issue) -> tuple[int, ...]:
        """The cone's population per configured priority, highest first, with a
        trailing bucket for unconfigured ones. Compared lexicographically this is
        the whole ranking rule: the first non-zero slot *is* the cone's maximum
        priority (blocking an urgent issue beats being high), and within a slot a
        larger count wins (blocking two high issues beats blocking one). Levels
        never trade, so no pile of mediums adds up to a high."""
        # One slot per priority plus a trailing bucket, which `priority_rank` sorts an
        # unrecognised value into — the one way junk still reaches here is a hand edit.
        counts = [0] * (len(PRIORITIES) + 1)
        for i in self.demand_cone(r):
            counts[priority_rank(self.cfg, self.by_id[i].priority)] += 1
        return tuple(counts)

    def demand_source(self, r: Issue) -> Issue | None:
        """The cone member that makes `r` rank above its own priority — the
        highest-priority issue waiting on it, or None when `r` is already the
        maximum (nothing to explain). Ties go to the lowest id, so the note a row
        carries is stable across runs."""
        own = priority_rank(self.cfg, r.priority)
        best = None
        for i in sorted(self.demand_cone(r)):
            if i == r.id:
                continue
            rank = priority_rank(self.cfg, self.by_id[i].priority)
            if rank < own and (best is None or rank < priority_rank(self.cfg, best.priority)):
                best = self.by_id[i]
        return best

    # dependency graph
    def dependency_line(self, r: Issue, up: bool = True, down: bool = True,
                        hier: bool = True) -> set[int]:
        """The ids in `r`'s directed dependency line: `r` itself, plus — when `up` —
        everything it transitively depends on (prerequisites, following edges forward)
        and — when `down` — everything that transitively depends on it (dependents,
        following edges back). Excludes "cousins" joined only through a shared
        neighbour — unlike a weakly-connected component, the two sweeps never cross
        direction. `up`/`down` gate each sweep so callers can scope to one cone.

        With `hier`, containment edges are followed too: `up` from a parent descends
        its whole subtree (what it is waiting on), `down` from a child climbs to the
        parents that contain it. Siblings remain cousins — they meet only at the
        parent, and neither sweep crosses direction."""
        seen = {r.id}
        if up:
            stack = [r.id]                              # forward: prerequisites
            while stack:
                node = self.by_id.get(stack.pop())
                if node:
                    for d, _kind in self.drawn_deps_of(node, hier):
                        if d.id not in seen:
                            seen.add(d.id); stack.append(d.id)
        if down:
            stack = [r.id]                              # back: dependents
            while stack:
                node = self.by_id.get(stack.pop())
                if node:
                    for dep in self.drawn_dependents_of(node, hier):
                        if dep.id not in seen:
                            seen.add(dep.id); stack.append(dep.id)
        return seen

    def containment(self, a: int, b: int) -> str | None:
        """The parent-spine relationship between `a` and `b`, or None when their
        subtrees are disjoint. `"same"` (a == b), `"descendant"` (a is a descendant
        of b), `"ancestor"` (a is an ancestor of b). A dependency edge is admissible
        only when this returns None — a shared subtree self-cycles under lifting. A
        local O(depth) check (walks one spine)."""
        if a == b:
            return "same"
        ra, rb = self.by_id.get(a), self.by_id.get(b)
        if ra is None or rb is None:
            return None
        if any(x.id == b for x in self.ancestors_of(ra)):
            return "descendant"
        if any(x.id == a for x in self.ancestors_of(rb)):
            return "ancestor"
        return None

    def _eff_reach(self, start_ids) -> set:
        """The ids effectively depended-on (transitively) by any node in
        `start_ids`, under the lifting rule: from each node climb its spine to
        inherit authored deps (`lifted_deps`), then expand each dep's `subtree`.
        Restarting from every target chains hops correctly."""
        seen, stack = set(), list(start_ids)
        while stack:
            node = self.by_id.get(stack.pop())
            if node is None:
                continue
            for b in self.lifted_deps(node):
                for t in self.subtree(b):
                    if t.id not in seen:
                        seen.add(t.id)
                        stack.append(t.id)
        return seen

    def would_cycle(self, src: int, dep: int) -> bool:
        """True if adding the edge src->dep (src depends_on dep) would create an
        *effective* dependency cycle — one implied through the parent hierarchy, not
        only the authored edges. The new edge lifts to subtree(src) ⇒ subtree(dep),
        so it closes a loop iff some node in subtree(dep) already effectively reaches
        some node in subtree(src). Generalizes the plain reachability check."""
        if src == dep:
            return True
        src_row, dep_row = self.by_id.get(src), self.by_id.get(dep)
        if src_row is None or dep_row is None:
            return False
        src_ids = {n.id for n in self.subtree(src_row)}
        reached = self._eff_reach(n.id for n in self.subtree(dep_row))
        return bool(reached & src_ids)

    def _effective_adj(self) -> dict:
        """Every effective direct-dependency edge as `x -> [(target, (a, b)), …]`,
        where `(a, b)` is the authored edge (a ∈ ancestors(x)+x, target ∈ subtree(b))
        that induces it. The witness lets `validate`/guards name the authored edges +
        parent links behind an implied cycle."""
        adj: dict = {r.id: [] for r in self.rows}
        for x in self.rows:
            for a in (x, *self.ancestors_of(x)):
                for d in a.depends_on:
                    b = self.by_id.get(d)
                    if b is None:
                        continue
                    for t in self.subtree(b):
                        adj[x.id].append((t.id, (a.id, b.id)))
        return adj

    def effective_cycles(self) -> list[list[int]]:
        """Every cycle in the *effective* dependency graph (authored edges lifted
        through the parent hierarchy), each reported once. A superset of `cycles()`:
        an authored cycle is also an effective one. Used by `validate`."""
        adj = self._effective_adj()
        succ = {i: [t for (t, _) in edges] for i, edges in adj.items()}
        return self._find_cycles(lambda r: succ.get(r.id, ()))

    def describe_effective_cycle(self, cyc: list[int]) -> str:
        """A human-readable reason for an effective cycle `cyc`: the node loop plus
        the authored edges and parent links that induce it (the loop itself is
        implied, never typed by the user)."""
        adj = self._effective_adj()
        chain = " -> ".join(f"#{c}" for c in (*cyc, cyc[0]))
        seq = [*cyc, cyc[0]]
        authored, notes = [], []
        for u, v in zip(seq, seq[1:]):
            wit = next(((a, b) for (t, (a, b)) in adj.get(u, []) if t == v), None)
            if wit is None:
                continue
            a, b = wit
            edge = f"#{a} -> #{b}"
            if edge not in authored:
                authored.append(edge)
            if a != u:
                notes.append(f"#{u} inherits #{a}'s deps")
            if b != v:
                notes.append(f"#{v} is under #{b}")
        reason = chain
        if authored:
            reason += "; authored: " + ", ".join(authored)
        if notes:
            reason += "; " + ", ".join(dict.fromkeys(notes))
        return reason

    def _find_cycles(self, successors) -> list[list[int]]:
        """Every cycle reachable by following `successors(row)` (a list of next ids)
        as a list of node ids, each cycle reported once (deduped by its node set).
        DFS with white/gray/black coloring; a self-edge is a one-node cycle. Shared
        by `cycles` (depends_on edges) and `parent_cycles` (the single parent edge)."""
        by_id = self.by_id
        color = {iid: 0 for iid in by_id}  # 0=unseen, 1=on-stack, 2=done
        found, seen_keys, path = [], set(), []

        def visit(node):
            color[node] = 1
            path.append(node)
            for nxt in successors(by_id[node]):
                if nxt not in by_id:
                    continue
                if color[nxt] == 1:
                    cyc = path[path.index(nxt):]
                    key = frozenset(cyc)
                    if key not in seen_keys:
                        seen_keys.add(key)
                        found.append(list(cyc))
                elif color[nxt] == 0:
                    visit(nxt)
            path.pop()
            color[node] = 2

        for iid in by_id:
            if color[iid] == 0:
                visit(iid)
        return found

    def cycles(self) -> list[list[int]]:
        """Every cycle in the depends_on graph, each reported once."""
        return self._find_cycles(lambda r: r.depends_on)

    def parent_cycles(self) -> list[list[int]]:
        """Every cycle in the parent graph, each reported once. The parent graph is
        functional (each node has at most one parent), so cycles are simple and
        disjoint; a self-parent (#n parent #n) is a one-node cycle. Nodes that merely
        point into a cycle are not part of it and are not reported."""
        return self._find_cycles(lambda r: () if r.parent is None else (r.parent,))


def edge_reach(edges: dict) -> dict:
    """`{node: every node reachable from it}` over an `{u: [(v, kind), …]}` edge map.
    Memoised DFS; the placeholder written before recursing makes it terminate on a
    malformed cycle instead of blowing the stack."""
    reach: dict = {}

    def below(u):
        if u not in reach:
            reach[u] = set()                           # guards a malformed cycle
            acc = set()
            for v, _k in edges.get(u, ()):
                acc.add(v); acc |= below(v)
            reach[u] = acc
        return reach[u]

    for u in edges:
        below(u)
    return reach


def implied_edges(edges: dict):
    """Every `(u, v, w)` where the edge `u -> v` is already implied by `u -> w -> … -> v`.
    On a DAG these are exactly the edges a transitive reduction removes, and `w` is a
    witness — the covering path's first hop, which is what makes a report actionable."""
    reach = edge_reach(edges)
    for u, targets in edges.items():
        for v, _kind in targets:
            for w, _k in targets:
                if w != v and v in reach.get(w, ()):
                    yield u, v, w
                    break


def transitive_reduction(edges: dict) -> dict:
    """Drop every edge already implied by a longer path: `u -> v` goes if some other
    `u -> w` reaches `v`. On a DAG the result is unique and preserves reachability
    exactly, so there is no arbitrary choice to make and nothing is lost — the path
    that justified the removal is still drawn.

    Display-only. The authored edge stays in the index; only `dep --remove` deletes
    one. `check` is where redundancy gets *reported* rather than silently hidden."""
    cut = {}
    for u, v, _w in implied_edges(edges):
        cut.setdefault(u, set()).add(v)
    return {u: [(v, k) for v, k in targets if v not in cut.get(u, ())]
            for u, targets in edges.items()}


def load_graph(ctx: Ctx) -> Graph:
    """Load the index and wrap it in a `Graph` — the read-side analog of `load_index`."""
    return Graph(ctx.cfg, load_index(ctx))


def normalize_points(g: Graph) -> None:
    """`points` is a leaf-only input. A node with children has no own weight — its
    points are derived from its leaves — so reset it to the default (which serializes
    as nothing). Mutates rows in place; called on every index write via finalize."""
    for r in g.rows:
        if not g.is_leaf(r):
            r.points = DEFAULT_POINTS


def _postorder(g: Graph) -> list[Issue]:
    """Rows ordered children-before-parents, so a bottom-up pass sees each node's
    descendants already settled. A `seen` guard makes it safe on a malformed parent
    cycle (which `validate` reports separately)."""
    order, seen = [], set()
    def visit(r: Issue) -> None:
        if r.id in seen:
            return
        seen.add(r.id)
        for c in g.children_of(r):
            visit(c)
        order.append(r)
    for r in g.rows:
        visit(r)
    return order


def normalize_statuses(ctx: Ctx, g: Graph) -> None:
    """A parent's status is a rollup of its children's (#67): all-initial -> initial,
    all-terminal -> terminal, otherwise active. Like `normalize_points`, this derives
    the rollup on every index write via finalize — so cascade is uniform across mv /
    start / done / new --parent / reparent with no per-command hooks. Walks bottom-up
    and restatuses changed parents through `apply_status` — date stamping, no file
    contact, because this derives a value rather than carrying out an instruction
    about a specific issue. Staying pure keeps it usable where the working tree is
    not settled (a merge driver runs mid-operation). Nodes flagged `manual_status`
    are an explicit opt-out and are left untouched."""
    for r in _postorder(g):
        kids = g.children_of(r)
        if not kids or r.manual_status:
            continue
        desired = reconcile(ctx.cfg, [k.status for k in kids])
        if desired and desired != r.status:
            apply_status(ctx.cfg, r, desired)


def _existing_ids(ctx: Ctx) -> set[str]:
    """Every id currently visible: index rows ∪ on-disk filenames."""
    ids = {r.id for r in load_index(ctx)}
    d = ctx.dir / ITEMS_DIR
    if d.is_dir():
        for p in d.glob("*.md"):
            m = FILENAME_RE.match(p.name)
            if m:
                ids.add(file_id(m))
    return ids


def gen_id(ctx: Ctx) -> str:
    """A fresh random id. Within-branch guard: redraw if it collides with any id
    already visible in the index or on disk; the unseen cross-branch tail stays
    optimistic (collision improbable, not impossible).

    All-digit candidates used to be redrawn, to keep `all-digit ⇔ legacy integer
    id` sound. Nothing reads that discriminator now, so `2345678` is an ordinary
    id and the alphabet is used in full."""
    seen = _existing_ids(ctx)
    while True:
        cand = "".join(secrets.choice(ID_ALPHABET) for _ in range(ID_LEN))
        if cand not in seen:
            return cand


