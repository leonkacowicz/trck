//! Which arrows the gutter is asked to draw, before any of them are reduced away or laid
//! out.
//!
//! Three sources, and they are not symmetric: a node's own `depends_on`, one edge per
//! child because a parent is done exactly when its children are, and the dependencies its
//! ancestors authored — which is where the altitude question lives.

use super::{EdgeKind, Edges};
use crate::graph::Graph;
use std::collections::{BTreeMap, BTreeSet};

/// The drawn edge set restricted to `ids`, transitively reduced when `reduce`.
pub(crate) fn drawn_edges(g: &Graph, ids: &BTreeSet<String>, reduce: bool, fanout: bool) -> Edges {
    let mut edges: Edges = BTreeMap::new();
    for id in ids {
        let mut out = authored(g, id, ids);
        out.extend(inherited(g, id, ids, fanout));
        edges.insert(id.clone(), out);
    }
    if reduce { super::reduce::transitive_reduction(&edges) } else { edges }
}

/// The edges `id` states for itself: its own prerequisites, plus one per child.
fn authored(g: &Graph, id: &str, ids: &BTreeSet<String>) -> Vec<(String, EdgeKind)> {
    let mut out: Vec<(String, EdgeKind)> = g.requires_of(id).into_iter().filter(|d| ids.contains(d)).map(|d| (d, EdgeKind::Dep)).collect();
    out.extend(g.children_of(id).iter().filter(|kid| ids.contains(*kid)).map(|kid| (kid.clone(), EdgeKind::Child)));
    out
}

/// Dependencies an ancestor authored, drawn under `id` itself.
///
/// One is dropped when an ancestor between the node and the issue that authored it is on
/// screen: that row already carries the dependency, and the containment edges connect the
/// two. Restating it under each child would replace one parent-altitude edge with a fan of
/// n, and reduction would then delete the parent's own edge as implied by its children —
/// so suppressing the fan up front is what keeps a dependency at the altitude it was
/// authored. `fanout` asks for the fan anyway.
fn inherited(g: &Graph, id: &str, ids: &BTreeSet<String>, fanout: bool) -> Vec<(String, EdgeKind)> {
    // Its own prerequisites are already drawn, and seeing a target once is enough: the
    // nearest ancestor that authored it is the one that gets the edge.
    let mut seen: BTreeSet<String> = g.requires_of(id).into_iter().collect();
    let mut out = Vec::new();
    for author in g.ancestors_of(id) {
        // Independent of the target, so it is asked once per ancestor rather than once per
        // edge that ancestor authored.
        let quiet = !fanout && carried_above(g, id, &author, ids);
        for target in g.requires_of(&author) {
            if seen.insert(target.clone()) && !quiet && ids.contains(&target) {
                out.push((target, EdgeKind::Inherited));
            }
        }
    }
    out
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
