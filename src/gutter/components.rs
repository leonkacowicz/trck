//! Splitting the drawn graph into blocks that can be laid out independently.

use super::Edges;
use std::collections::{BTreeMap, BTreeSet};

/// Who each node is connected to, ignoring edge direction.
type Adjacent<'a> = BTreeMap<&'a str, BTreeSet<&'a str>>;

/// Weakly-connected components over the drawn edges, each id-sorted, ordered by smallest
/// member — so a node's cluster renders as one contiguous, separable block.
pub(crate) fn components(ids: &BTreeSet<String>, edges: &Edges) -> Vec<Vec<String>> {
    let adj = undirected(ids, edges);
    let mut seen: BTreeSet<&str> = BTreeSet::new();
    let mut comps: Vec<Vec<String>> = Vec::new();
    for start in ids {
        if seen.insert(start.as_str()) {
            comps.push(reachable(start.as_str(), &adj, &mut seen));
        }
    }
    comps.sort_by(|a, b| a.first().cmp(&b.first()));
    comps
}

/// Both ends of every drawn edge that has both ends on screen. Direction is dropped
/// deliberately: this is about which nodes belong in the same block, not about ordering
/// them — that is [`super::order`]'s job.
fn undirected<'a>(ids: &'a BTreeSet<String>, edges: &'a Edges) -> Adjacent<'a> {
    let mut adj: Adjacent<'a> = ids.iter().map(|i| (i.as_str(), BTreeSet::new())).collect();
    for (u, targets) in edges.iter().filter(|(u, _)| ids.contains(*u)) {
        for (v, _) in targets.iter().filter(|(v, _)| ids.contains(v)) {
            adj.entry(u.as_str()).or_default().insert(v.as_str());
            adj.entry(v.as_str()).or_default().insert(u.as_str());
        }
    }
    adj
}

/// Everything reachable from `start`, id-sorted, marking each id seen as it is taken — so
/// the caller's scan skips a node already claimed by an earlier component.
fn reachable<'a>(start: &'a str, adj: &Adjacent<'a>, seen: &mut BTreeSet<&'a str>) -> Vec<String> {
    let mut comp = vec![start.to_string()];
    let mut stack = vec![start];
    while let Some(x) = stack.pop() {
        for y in adj.get(x).into_iter().flatten() {
            if seen.insert(*y) {
                comp.push((*y).to_string());
                stack.push(*y);
            }
        }
    }
    comp.sort();
    comp
}
