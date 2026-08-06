//! The dependency DAG as a lazygit-style gutter: one row per node, every merge and fork
//! drawn on its own node's row as box-drawing corners rather than on blank edge rows.
//!
//! A lane opens at a prerequisite's row and closes at its dependent's, so each lane
//! column carries exactly one edge — which is why the edge *kind* rides along with the
//! lane and an inferred containment edge can be drawn differently from an authored one.
//!
//! Three things happen before any drawing, and the order matters:
//!
//! 1. **The edge set is reduced** over exactly the ids being drawn, so an edge is only
//!    ever dropped in favour of a path that is itself on screen.
//! 2. **A topological order** puts prerequisites first, tie-broken by locality so a
//!    branch is finished before the next one starts.
//! 3. **Lanes are shortened** by local search, because locality is a local rule: it
//!    settles each step without seeing what the choice costs further down, and strands a
//!    root whose only dependent is far below.

use crate::config::is_terminal;
use crate::graph::Graph;
mod canvas;

use canvas::Canvas;
use std::collections::{BTreeMap, BTreeSet};

/// The kind of a drawn edge. Authored dependencies and inferred containment are drawn
/// differently, so the lane carries which it is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(crate) enum EdgeKind {
    /// An authored `depends_on`.
    Dep,
    /// A parent waiting on a child. Nobody authors these; they follow from containment.
    Child,
    /// A dependency lifted from an ancestor.
    Inherited,
}

/// A cell's connections, mapped to a box-drawing glyph.
fn glyph(dirs: &BTreeSet<char>) -> char {
    let key: String = dirs.iter().collect();
    match key.as_str() {
        "DU" | "D" | "U" => '│',
        "LR" | "L" | "R" => '─',
        "RU" => '╰',
        "LU" => '╯',
        "DR" => '╭',
        "DL" => '╮',
        "DRU" => '├',
        "DLU" => '┤',
        "LRU" => '┴',
        "DLR" => '┬',
        "DLRU" => '┼',
        _ => ' ',
    }
}

/// Every node reachable from each node, memoised. The placeholder written before
/// recursing is what makes it terminate on a malformed cycle instead of blowing the
/// stack.
fn edge_reach(edges: &BTreeMap<String, Vec<(String, EdgeKind)>>) -> BTreeMap<String, BTreeSet<String>> {
    let mut reach: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for start in edges.keys() {
        if reach.contains_key(start) {
            continue;
        }
        // Iterative post-order, so a deep chain cannot overflow the stack.
        let mut stack = vec![(start.clone(), false)];
        while let Some((u, expanded)) = stack.pop() {
            if expanded {
                let mut acc = BTreeSet::new();
                for (v, _) in edges.get(&u).into_iter().flatten() {
                    acc.insert(v.clone());
                    if let Some(below) = reach.get(v) {
                        acc.extend(below.iter().cloned());
                    }
                }
                reach.insert(u, acc);
                continue;
            }
            if reach.contains_key(&u) {
                continue;
            }
            reach.insert(u.clone(), BTreeSet::new()); // guards a malformed cycle
            stack.push((u.clone(), true));
            for (v, _) in edges.get(&u).into_iter().flatten() {
                stack.push((v.clone(), false));
            }
        }
    }
    reach
}

/// Drop every edge already implied by a longer path.
///
/// Display-only: the authored edge stays in the index, and only `dep --remove` deletes
/// one. On a DAG the result is unique and preserves reachability exactly, so nothing is
/// lost — the path that justified the removal is still drawn.
fn transitive_reduction(edges: &BTreeMap<String, Vec<(String, EdgeKind)>>) -> BTreeMap<String, Vec<(String, EdgeKind)>> {
    let reach = edge_reach(edges);
    let mut out = BTreeMap::new();
    for (u, targets) in edges {
        let kept: Vec<(String, EdgeKind)> =
            targets.iter().filter(|(v, _)| !targets.iter().any(|(w, _)| w != v && reach.get(w).is_some_and(|r| r.contains(v)))).cloned().collect();
        out.insert(u.clone(), kept);
    }
    out
}

/// The drawn edge set restricted to `ids`.
///
/// An inherited edge is dropped when an ancestor between the node and the issue that
/// authored it is on screen: that row already carries the dependency, and the containment
/// edges connect the two. Restating it under each child would replace one
/// parent-altitude edge with a fan of n, and reduction would then delete the parent's own
/// edge as implied by its children — so suppressing the fan up front is what keeps a
/// dependency at the altitude it was authored.
pub(crate) fn drawn_edges(g: &Graph, ids: &BTreeSet<String>, reduce: bool, fanout: bool) -> BTreeMap<String, Vec<(String, EdgeKind)>> {
    let mut edges: BTreeMap<String, Vec<(String, EdgeKind)>> = BTreeMap::new();
    for id in ids {
        let mut out: Vec<(String, EdgeKind)> = g.requires_of(id).into_iter().filter(|d| ids.contains(d)).map(|d| (d, EdgeKind::Dep)).collect();
        for kid in g.children_of(id) {
            if ids.contains(kid) {
                out.push((kid.clone(), EdgeKind::Child));
            }
        }
        // Inherited: a target visible through the spine that this node did not author.
        let own: BTreeSet<String> = g.requires_of(id).into_iter().collect();
        let mut seen: BTreeSet<String> = own.clone();
        for author in g.ancestors_of(id) {
            for target in g.requires_of(&author) {
                if !seen.insert(target.clone()) || !ids.contains(&target) {
                    continue;
                }
                if !fanout && carried_above(g, id, &author, ids) {
                    continue;
                }
                out.push((target, EdgeKind::Inherited));
            }
        }
        edges.insert(id.clone(), out);
    }
    if reduce { transitive_reduction(&edges) } else { edges }
}

/// Is a drawn row between `id` and `author` (inclusive) already saying it?
fn carried_above(g: &Graph, id: &str, author: &str, ids: &BTreeSet<String>) -> bool {
    for a in g.ancestors_of(id) {
        if ids.contains(&a) {
            return true;
        }
        if a == author {
            break;
        }
    }
    false
}

/// Weakly-connected components over the drawn edges, each id-sorted, ordered by smallest
/// member — so a node's cluster renders as one contiguous, separable block.
pub(crate) fn components(ids: &BTreeSet<String>, edges: &BTreeMap<String, Vec<(String, EdgeKind)>>) -> Vec<Vec<String>> {
    let mut adj: BTreeMap<&str, BTreeSet<&str>> = ids.iter().map(|i| (i.as_str(), BTreeSet::new())).collect();
    for (u, targets) in edges {
        for (v, _) in targets {
            if ids.contains(u) && ids.contains(v) {
                adj.entry(u.as_str()).or_default().insert(v.as_str());
                adj.entry(v.as_str()).or_default().insert(u.as_str());
            }
        }
    }
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut comps: Vec<Vec<String>> = Vec::new();
    for start in ids {
        if !seen.insert(start.as_str()) {
            continue;
        }
        let mut comp = vec![start.clone()];
        let mut stack = vec![start.as_str()];
        while let Some(x) = stack.pop() {
            for y in adj.get(x).into_iter().flatten() {
                if seen.insert(y) {
                    comp.push((*y).to_string());
                    stack.push(y);
                }
            }
        }
        comp.sort();
        comps.push(comp);
    }
    comps.sort_by(|a, b| a.first().cmp(&b.first()));
    comps
}

/// Slide single nodes along the order to shorten the gutter, keeping it a linear
/// extension.
///
/// The cost is total lane length: a lane opens at its prerequisite's row and closes at
/// its dependent's, so the sum of those spans is exactly what fills the gutter with idle
/// `│`. Prerequisites-first alone does not care — it will happily emit a root whose only
/// dependent is at the bottom, leaving its lane open the whole way down.
///
/// Gathered per node rather than per edge, the cost collapses to
/// `sum(pos[v] * (indeg[v] - outdeg[v]))` — linear in the positions. That is what makes
/// this cheap: moving one node shifts a contiguous block by exactly one row, so a
/// candidate's delta reads off a prefix sum in constant time instead of costing a walk
/// over the edges.
///
/// First improvement, repeated until nothing helps. Termination needs no iteration cap:
/// the cost is a non-negative integer and every accepted move drops it by at least one.
fn shorten_lanes(order: &mut Vec<String>, pairs: &[(String, String)]) {
    // Owned keys throughout: the search mutates `order`, so borrowing its strings would
    // pin it for the whole function.
    let n = order.len();
    let mut weight: BTreeMap<String, i64> = order.iter().map(|v| (v.clone(), 0)).collect();
    let mut after: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut before: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (a, b) in pairs {
        *weight.entry(b.clone()).or_default() += 1;
        *weight.entry(a.clone()).or_default() -= 1;
        after.entry(b.clone()).or_default().insert(a.clone());
        before.entry(a.clone()).or_default().insert(b.clone());
    }
    let prefix = |order: &[String], weight: &BTreeMap<String, i64>| -> Vec<i64> {
        let mut out = vec![0i64; order.len() + 1];
        let mut run = 0;
        for (k, v) in order.iter().enumerate() {
            run += weight.get(v).copied().unwrap_or(0);
            out[k + 1] = run;
        }
        out
    };

    loop {
        let mut moved = false;
        let mut at: BTreeMap<String, usize> = order.iter().enumerate().map(|(k, v)| (v.clone(), k)).collect();
        let mut pref = prefix(order, &weight);
        // The scan continues from i+1 over the already-updated order rather than
        // restarting on every accepted move. Restarting reaches a different local
        // optimum — same cost function, different fixed point — and this is the one the
        // goldens were written against.
        for i in 0..n {
            let u = order[i].clone();
            let w = weight.get(&u).copied().unwrap_or(0);
            // Between the last prerequisite and the first dependent: every slot in that
            // window keeps the order a linear extension, and no slot outside it does.
            let lo = after.get(&u).into_iter().flatten().filter_map(|p| at.get(p)).max().map_or(0, |m| m + 1);
            let hi = before.get(&u).into_iter().flatten().filter_map(|d| at.get(d)).min().map_or(n, |m| *m).saturating_sub(1);
            for j in lo..=hi.min(n.saturating_sub(1)) {
                if j == i {
                    continue;
                }
                // `u` travels j - i rows; everything it steps over shifts one row the
                // other way, and the prefix sum totals that block's weight at once.
                let span = i64::try_from(j).unwrap_or(0) - i64::try_from(i).unwrap_or(0);
                let delta = if j > i { w * span - (pref[j + 1] - pref[i + 1]) } else { w * span + (pref[i] - pref[j]) };
                if delta < 0 {
                    let node = order.remove(i);
                    order.insert(j, node);
                    at = order.iter().enumerate().map(|(k, v)| (v.clone(), k)).collect();
                    pref = prefix(order, &weight);
                    moved = true;
                    break;
                }
            }
        }
        if !moved {
            return;
        }
    }
}

/// Topological order over the drawn edges, prerequisites first, plus each node's
/// id-sorted dependents and the edge kinds.
///
/// Tie-break is depth-first by locality: among ready nodes take the one unblocked *most
/// recently*, so a branch is drawn to its end before the next starts and its lane closes
/// on the next row instead of lingering beside a parallel branch. Siblings unblocked
/// together go in ascending id order, so the layout is fully deterministic.
fn topo(comp: &[String], edges: &Edges) -> Topo {
    let idset: BTreeSet<&str> = comp.iter().map(String::as_str).collect();
    let mut requires: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let mut kinds: BTreeMap<(String, String), EdgeKind> = BTreeMap::new();
    let mut dependents: BTreeMap<String, Vec<String>> = comp.iter().map(|i| (i.clone(), Vec::new())).collect();
    for i in comp {
        let targets: Vec<String> = edges
            .get(i)
            .into_iter()
            .flatten()
            .filter(|(d, _)| idset.contains(d.as_str()))
            .map(|(d, k)| {
                kinds.insert((i.clone(), d.clone()), *k);
                d.clone()
            })
            .collect();
        for d in &targets {
            dependents.entry(d.clone()).or_default().push(i.clone());
        }
        requires.insert(i.as_str(), targets);
    }
    for v in dependents.values_mut() {
        v.sort();
    }
    let mut indeg: BTreeMap<&str, usize> = comp.iter().map(|i| (i.as_str(), requires.get(i.as_str()).map_or(0, Vec::len))).collect();
    // A LIFO stack is the depth-first part; pushing newly-ready nodes highest-id-first
    // leaves the lowest on top, so a freshly-unblocked set is visited in ascending order.
    let mut stack: Vec<String> = comp.iter().filter(|i| indeg.get(i.as_str()) == Some(&0)).cloned().collect();
    stack.reverse();
    let mut order: Vec<String> = Vec::new();
    while let Some(n) = stack.pop() {
        order.push(n.clone());
        let mut newly: Vec<String> = Vec::new();
        for dep in dependents.get(&n).into_iter().flatten() {
            if let Some(d) = indeg.get_mut(dep.as_str()) {
                *d -= 1;
                if *d == 0 {
                    newly.push(dep.clone());
                }
            }
        }
        newly.sort();
        newly.reverse();
        stack.extend(newly);
    }
    let pairs: Vec<(String, String)> = comp.iter().flat_map(|i| requires.get(i.as_str()).into_iter().flatten().map(move |d| (d.clone(), i.clone()))).collect();
    shorten_lanes(&mut order, &pairs);
    (order, dependents, kinds)
}

/// Which edge a gutter cell belongs to, for colouring. `None` where nothing owns it.
pub(crate) type LaneOwner = Option<(String, EdgeKind)>;

/// One rendered row: the issue id, the gutter, and a per-character lane owner.
pub(crate) type Row = (String, String, Vec<LaneOwner>);

/// The drawn edge set: source -> its targets, each with the kind of edge.
type Edges = BTreeMap<String, Vec<(String, EdgeKind)>>;

/// A topological order plus what the renderer needs alongside it.
type Topo = (Vec<String>, BTreeMap<String, Vec<String>>, BTreeMap<(String, String), EdgeKind>);

/// Render one connected component, one row per node.
fn component_rows(comp: &[String], edges: &Edges) -> Vec<Row> {
    let (order, dependents, kinds) = topo(comp, edges);
    let mut lanes: Vec<LaneOwner> = Vec::new();
    let mut rows = Vec::new();

    for n in &order {
        let top = lanes.clone();
        let arriving: Vec<usize> = top.iter().enumerate().filter(|(_, t)| t.as_ref().is_some_and(|(d, _)| d == n)).map(|(c, _)| c).collect();
        let pos = arriving.first().copied().unwrap_or_else(|| top.iter().position(Option::is_none).unwrap_or(top.len()));
        let mut bottom = top.clone();
        while bottom.len() <= pos {
            bottom.push(None);
        }
        for c in &arriving {
            bottom[*c] = None;
        }
        bottom[pos] = None;

        let mut started: Vec<usize> = Vec::new();
        for (k, d) in dependents.get(n).into_iter().flatten().enumerate() {
            let lane = (d.clone(), kinds.get(&(d.clone(), n.clone())).copied().unwrap_or(EdgeKind::Dep));
            if k == 0 {
                bottom[pos] = Some(lane);
                started.push(pos);
            } else {
                // Reuse the free column *nearest* the node, not the leftmost gap: the
                // same lane count, but a shorter horizontal bridge and fewer crossings.
                // Ties go to the lower column.
                let free: Vec<usize> = bottom.iter().enumerate().filter(|(_, t)| t.is_none()).map(|(c, _)| c).collect();
                let c = free.iter().min_by_key(|c| (c.abs_diff(pos), **c)).copied().unwrap_or(bottom.len());
                if c == bottom.len() {
                    bottom.push(None);
                }
                bottom[c] = Some(lane);
                started.push(c);
            }
        }

        rows.push(draw_row(n, &top, &bottom, pos, &arriving, &started));
        while bottom.last().is_some_and(Option::is_none) {
            bottom.pop();
        }
        lanes = bottom;
    }
    rows
}

fn draw_row(n: &str, top: &[LaneOwner], bottom: &[LaneOwner], pos: usize, arriving: &[usize], started: &[usize]) -> Row {
    let width = top.len().max(bottom.len()).max(pos + 1);
    let mut canvas = Canvas::new(width, pos);
    canvas.through(top, arriving);
    for a in arriving {
        canvas.connect(*a, &top.get(*a).cloned().flatten(), 'U');
    }
    for b in started {
        canvas.connect(*b, &bottom.get(*b).cloned().flatten(), 'D');
    }
    canvas.render(n)
}

/// Render the DAG over `ids`, grouped by component with a `None` separator between
/// groups.
///
/// The edge set is built and reduced here, over exactly the ids being drawn. Doing it at
/// this point rather than at the caller is what makes the ordering trap impossible:
/// `ids` has already been done-filtered, so an edge can never be dropped in favour of a
/// path through a node that is not rendered — which would leave its endpoints looking
/// unrelated.
pub(crate) fn render_graph(g: &Graph, ids: &BTreeSet<String>, fanout: bool) -> Vec<Option<Row>> {
    let edges = drawn_edges(g, ids, true, fanout);
    let unreduced = drawn_edges(g, ids, false, fanout);
    let mut out: Vec<Option<Row>> = Vec::new();
    for comp in components(ids, &unreduced) {
        if !out.is_empty() {
            out.push(None);
        }
        out.extend(component_rows(&comp, &edges).into_iter().map(Some));
    }
    out
}

/// The id set for the bare `deps` view: every component holding at least one *authored*
/// edge, taken whole.
///
/// Containment edges connect nearly the whole forest, so "every issue touching an edge"
/// would match almost everything and turn this view into `list`. Selecting by authored
/// edges keeps it about ordering constraints. Components are kept or dropped *whole*: a
/// parent shown without some of its children would misreport what it is waiting on,
/// which is precisely the question this view exists to answer.
pub(crate) fn overview_ids(g: &Graph) -> BTreeSet<String> {
    let all: BTreeSet<String> = g.rows.iter().map(|r| r.id.clone()).collect();
    let edges = drawn_edges(g, &all, false, false);
    let mut keep = BTreeSet::new();
    for comp in components(&all, &edges) {
        let members: BTreeSet<&str> = comp.iter().map(String::as_str).collect();
        let authored = comp.iter().any(|i| g.get(i).is_some_and(|r| r.depends_on.iter().any(|d| members.contains(d.as_str()))));
        if authored {
            keep.extend(comp);
        }
    }
    keep
}

/// Display-only done filtering. Fully terminal components are hidden only for the
/// whole-graph view; removing done nodes shrinks the id set, and the components are then
/// recomputed over the remaining subgraph so no synthetic edges appear across omitted
/// nodes.
pub(crate) fn filter_done(g: &Graph, ids: &BTreeSet<String>, omit_done: bool, include_done_chains: bool, hide_done_chains: bool) -> BTreeSet<String> {
    let mut kept = ids.clone();
    if hide_done_chains && !include_done_chains {
        let edges = drawn_edges(g, &kept, false, false);
        for comp in components(&kept, &edges) {
            if comp.iter().all(|i| g.get(i).is_some_and(|r| is_terminal(&r.status))) {
                for i in comp {
                    kept.remove(&i);
                }
            }
        }
    }
    if omit_done {
        kept.retain(|i| !g.get(i).is_some_and(|r| is_terminal(&r.status)));
    }
    kept
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn glyphs_cover_every_connection_set() {
        let set = |s: &str| s.chars().collect::<BTreeSet<char>>();
        assert_eq!(glyph(&set("UD")), '│');
        assert_eq!(glyph(&set("LR")), '─');
        assert_eq!(glyph(&set("UR")), '╰');
        assert_eq!(glyph(&set("UL")), '╯');
        assert_eq!(glyph(&set("DR")), '╭');
        assert_eq!(glyph(&set("DL")), '╮');
        assert_eq!(glyph(&set("UDLR")), '┼');
        assert_eq!(glyph(&set("")), ' ');
    }

    #[test]
    fn a_lone_blocker_slides_down_to_shorten_its_lane() {
        // `a` blocks only `d`; `b -> c -> d` is a chain. Prerequisites-first alone puts
        // `a` first (lowest id among the roots), leaving its lane open beside the whole
        // chain. Nothing forces that: it need only precede `d`.
        let mut order: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| (*s).to_string()).collect();
        let pairs: Vec<(String, String)> = [("a", "d"), ("b", "c"), ("c", "d")].iter().map(|(x, y)| ((*x).to_string(), (*y).to_string())).collect();
        shorten_lanes(&mut order, &pairs);
        assert_eq!(order, ["b", "c", "a", "d"]);
    }

    #[test]
    fn shortening_never_breaks_prerequisites_first() {
        let mut order: Vec<String> = ["a", "b", "c", "d", "e", "f"].iter().map(|s| (*s).to_string()).collect();
        let pairs: Vec<(String, String)> =
            [("a", "d"), ("b", "c"), ("c", "d"), ("a", "e"), ("d", "f"), ("e", "f")].iter().map(|(x, y)| ((*x).to_string(), (*y).to_string())).collect();
        shorten_lanes(&mut order, &pairs);
        for (a, b) in &pairs {
            let at = |x: &String| order.iter().position(|v| v == x).unwrap_or(usize::MAX);
            assert!(at(a) < at(b), "{a} must precede {b} in {order:?}");
        }
    }

    #[test]
    fn shortening_is_independent_of_the_input_order() {
        // The search runs off maps in places, so the guard that matters is that the
        // result does not depend on which order the ids arrived in.
        let pairs: Vec<(String, String)> = [("a", "d"), ("b", "c"), ("c", "d")].iter().map(|(x, y)| ((*x).to_string(), (*y).to_string())).collect();
        let mut first: Vec<String> = ["a", "b", "c", "d"].iter().map(|s| (*s).to_string()).collect();
        let mut second: Vec<String> = ["b", "a", "c", "d"].iter().map(|s| (*s).to_string()).collect();
        shorten_lanes(&mut first, &pairs);
        shorten_lanes(&mut second, &pairs);
        assert_eq!(first, second);
    }

    #[test]
    fn a_reduced_edge_is_dropped_only_when_the_path_is_drawn() {
        let mut edges: BTreeMap<String, Vec<(String, EdgeKind)>> = BTreeMap::new();
        edges.insert("c".into(), vec![("b".into(), EdgeKind::Dep), ("a".into(), EdgeKind::Dep)]);
        edges.insert("b".into(), vec![("a".into(), EdgeKind::Dep)]);
        edges.insert("a".into(), vec![]);
        let reduced = transitive_reduction(&edges);
        // c -> a is implied by c -> b -> a, so it goes; c -> b stays.
        assert_eq!(reduced["c"], vec![("b".to_string(), EdgeKind::Dep)]);
        assert_eq!(reduced["b"], vec![("a".to_string(), EdgeKind::Dep)]);
    }

    #[test]
    fn reduction_terminates_on_a_malformed_cycle() {
        // `check` reports cycles; the renderer must not hang before it gets there.
        let mut edges: BTreeMap<String, Vec<(String, EdgeKind)>> = BTreeMap::new();
        edges.insert("a".into(), vec![("b".into(), EdgeKind::Dep)]);
        edges.insert("b".into(), vec![("a".into(), EdgeKind::Dep)]);
        let _ = transitive_reduction(&edges);
    }
}
