//! Which row each node gets: prerequisites first, then [`super::shorten`] closes the lanes
//! that leaves open.

use super::shorten::shorten_lanes;
use super::{EdgeKind, Edges};
use std::collections::{BTreeMap, BTreeSet};

/// A topological order plus what the renderer needs alongside it.
pub(super) struct Topo {
    /// The nodes, in the order their rows are drawn.
    pub(super) order: Vec<String>,
    /// Each node's id-sorted dependents — the lanes that open beneath its row.
    pub(super) dependents: BTreeMap<String, Vec<String>>,
    /// The kind of each drawn edge, keyed by `(dependent, prerequisite)`.
    pub(super) kinds: BTreeMap<(String, String), EdgeKind>,
}

/// What each node requires, who depends on it, and the kind of every drawn edge.
type Adjacency = (BTreeMap<String, Vec<String>>, BTreeMap<String, Vec<String>>, BTreeMap<(String, String), EdgeKind>);

/// Topological order over the drawn edges, prerequisites first.
pub(super) fn topo(comp: &[String], edges: &Edges) -> Topo {
    let (requires, dependents, kinds) = adjacency(comp, edges);
    let mut order = depth_first(comp, &requires, &dependents);
    let pairs: Vec<(String, String)> = comp.iter().flat_map(|i| requires.get(i).into_iter().flatten().map(move |d| (d.clone(), i.clone()))).collect();
    shorten_lanes(&mut order, &pairs);
    Topo { order, dependents, kinds }
}

/// The three views of the edge set the layout needs, gathered in one pass because they all
/// come from the same walk. Edges leaving the component are skipped: it is laid out alone.
fn adjacency(comp: &[String], edges: &Edges) -> Adjacency {
    let inside: BTreeSet<&str> = comp.iter().map(String::as_str).collect();
    let mut requires: BTreeMap<String, Vec<String>> = BTreeMap::new();
    let mut dependents: BTreeMap<String, Vec<String>> = comp.iter().map(|i| (i.clone(), Vec::new())).collect();
    let mut kinds: BTreeMap<(String, String), EdgeKind> = BTreeMap::new();
    for i in comp {
        let mut targets: Vec<String> = Vec::new();
        for (d, k) in edges.get(i).into_iter().flatten().filter(|(d, _)| inside.contains(d.as_str())) {
            kinds.insert((i.clone(), d.clone()), *k);
            dependents.entry(d.clone()).or_default().push(i.clone());
            targets.push(d.clone());
        }
        requires.insert(i.clone(), targets);
    }
    for v in dependents.values_mut() {
        v.sort();
    }
    (requires, dependents, kinds)
}

/// Prerequisites first, tie-broken depth-first by locality: among ready nodes take the one
/// unblocked *most recently*, so a branch is drawn to its end before the next starts and
/// its lane closes on the next row instead of lingering beside a parallel branch.
///
/// A LIFO stack is the depth-first part. Pushing a newly-ready set highest-id-first leaves
/// the lowest on top, so siblings unblocked together are visited in ascending id order and
/// the layout is fully deterministic.
fn depth_first(comp: &[String], requires: &BTreeMap<String, Vec<String>>, dependents: &BTreeMap<String, Vec<String>>) -> Vec<String> {
    let mut indeg: BTreeMap<&str, usize> = comp.iter().map(|i| (i.as_str(), requires.get(i).map_or(0, Vec::len))).collect();
    let mut stack: Vec<String> = comp.iter().filter(|i| indeg.get(i.as_str()) == Some(&0)).cloned().collect();
    stack.reverse();
    let mut order: Vec<String> = Vec::new();
    while let Some(n) = stack.pop() {
        let mut newly: Vec<String> = Vec::new();
        for dep in dependents.get(&n).into_iter().flatten() {
            if let Some(d) = indeg.get_mut(dep.as_str()) {
                *d -= 1;
                if *d == 0 {
                    newly.push(dep.clone());
                }
            }
        }
        order.push(n);
        newly.sort();
        newly.reverse();
        stack.extend(newly);
    }
    order
}
