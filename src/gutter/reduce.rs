//! Dropping every edge a longer path already implies.
//!
//! Display-only: the authored edge stays in the index, and only `dep --remove` deletes
//! one. On a DAG the result is unique and preserves reachability exactly, so nothing is
//! lost — the path that justified the removal is still drawn.

use super::{EdgeKind, Edges};
use std::collections::{BTreeMap, BTreeSet};

/// Every node reachable from each node.
type Reach = BTreeMap<String, BTreeSet<String>>;

/// The edge set with the implied edges removed.
pub(super) fn transitive_reduction(edges: &Edges) -> Edges {
    let reach = edge_reach(edges);
    let mut out = BTreeMap::new();
    for (u, targets) in edges {
        let kept: Vec<(String, EdgeKind)> = targets.iter().filter(|(v, _)| !implied(v, targets, &reach)).cloned().collect();
        out.insert(u.clone(), kept);
    }
    out
}

/// Does a *sibling* edge out of the same node already reach `v`? That path is what makes
/// this edge redundant, and it is on screen because it is in this same edge set.
fn implied(v: &str, targets: &[(String, EdgeKind)], reach: &Reach) -> bool {
    targets.iter().any(|(w, _)| w.as_str() != v && reach.get(w).is_some_and(|r| r.contains(v)))
}

/// Reachability for every node, memoised across starts.
fn edge_reach(edges: &Edges) -> Reach {
    let mut reach = Reach::new();
    for start in edges.keys() {
        if !reach.contains_key(start) {
            fill_from(edges, start, &mut reach);
        }
    }
    reach
}

/// Fill `reach` for `start` and everything below it.
///
/// Iterative post-order, so a deep chain cannot overflow the stack. The placeholder
/// written before descending is what makes it terminate on a malformed cycle rather than
/// looping forever: `check` is what reports cycles, and the renderer must not hang before
/// the user gets there.
fn fill_from(edges: &Edges, start: &str, reach: &mut Reach) {
    let mut stack = vec![(start.to_string(), false)];
    while let Some((u, expanded)) = stack.pop() {
        if expanded {
            let below = union_below(edges, &u, reach);
            reach.insert(u, below);
        } else if !reach.contains_key(&u) {
            reach.insert(u.clone(), BTreeSet::new()); // guards a malformed cycle
            stack.push((u.clone(), true));
            for (v, _) in edges.get(&u).into_iter().flatten() {
                stack.push((v.clone(), false));
            }
        }
    }
}

/// A node's direct targets, plus whatever each of those already reaches.
fn union_below(edges: &Edges, u: &str, reach: &Reach) -> BTreeSet<String> {
    let mut acc = BTreeSet::new();
    for (v, _) in edges.get(u).into_iter().flatten() {
        acc.insert(v.clone());
        acc.extend(reach.get(v).into_iter().flatten().cloned());
    }
    acc
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn a_reduced_edge_is_dropped_only_when_the_path_is_drawn() {
        let mut edges: Edges = BTreeMap::new();
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
        let mut edges: Edges = BTreeMap::new();
        edges.insert("a".into(), vec![("b".into(), EdgeKind::Dep)]);
        edges.insert("b".into(), vec![("a".into(), EdgeKind::Dep)]);
        let _ = transitive_reduction(&edges);
    }

    #[test]
    fn a_chain_reaches_everything_below_it() {
        // Reachability is transitive, which is the whole basis for calling an edge implied:
        // the top of a chain must see the bottom, not just the next link.
        let mut edges: Edges = BTreeMap::new();
        for (a, b) in [("a", "b"), ("b", "c"), ("c", "d")] {
            edges.insert(a.into(), vec![(b.into(), EdgeKind::Dep)]);
        }
        edges.insert("d".into(), vec![]);
        let reach = edge_reach(&edges);
        assert_eq!(reach["a"], ["b", "c", "d"].iter().map(|s| (*s).to_string()).collect());
        assert_eq!(reach["c"], ["d"].iter().map(|s| (*s).to_string()).collect());
        assert!(reach["d"].is_empty());
    }
}
