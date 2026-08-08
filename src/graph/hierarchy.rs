//! Containment: the parent spine and what hangs off it.
//!
//! This is the structure the lifting rule climbs, so every other module here reaches for it.
//! Both walks are cycle-guarded: malformed data arrives mid-edit and must be reported, not
//! spun on.

use super::Graph;
use std::collections::BTreeSet;

impl Graph {
    /// `id`'s children, id-sorted. Containment says *what* composes a parent, not in
    /// what sequence, so there is no other order to preserve.
    pub(crate) fn children_of(&self, id: &str) -> &[String] {
        self.children.get(id).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn is_leaf(&self, id: &str) -> bool {
        !self.children.contains_key(id)
    }

    /// The parent spine above `id`, nearest first. A parent pointing at a missing id
    /// ends the spine, and a parent cycle is broken by the `seen` guard.
    pub(crate) fn ancestors_of(&self, id: &str) -> Vec<String> {
        let mut chain = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::from([id.to_string()]);
        let mut cur = id.to_string();
        while let Some(parent) = self.get(&cur).and_then(|r| r.parent.clone()) {
            if self.get(&parent).is_none() || !seen.insert(parent.clone()) {
                break;
            }
            chain.push(parent.clone());
            cur = parent;
        }
        chain
    }

    /// `id` plus every descendant. The target side of the lifting rule.
    pub(crate) fn subtree(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut stack = vec![id.to_string()];
        while let Some(n) = stack.pop() {
            if !seen.insert(n.clone()) {
                continue;
            }
            out.push(n.clone());
            stack.extend(self.children_of(&n).iter().cloned());
        }
        out
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use crate::test_graph::graph;

    #[test]
    fn hierarchy_walks_both_directions() {
        let g = graph(&["a", "b:a", "c:b", "d:a"]);
        assert_eq!(g.children_of("a"), ["b", "d"]);
        assert_eq!(g.ancestors_of("c"), ["b", "a"]);
        let mut sub = g.subtree("a");
        sub.sort();
        assert_eq!(sub, ["a", "b", "c", "d"]);
        assert!(g.is_leaf("c"));
        assert!(!g.is_leaf("a"));
    }

    #[test]
    fn a_parent_cycle_does_not_hang_the_walk() {
        // Malformed data reaches here mid-edit; it must be reported, not spun on.
        let g = graph(&["a:b", "b:a"]);
        assert!(g.ancestors_of("a").len() <= 2);
        assert!(!g.parent_cycles().is_empty());
    }

    /// A cycle must not hang the *downward* walk either — `subtree` has its own guard, and
    /// nothing else in the suite made it earn it.
    #[test]
    fn a_parent_cycle_does_not_hang_the_descent() {
        let g = graph(&["a:b", "b:a"]);
        let mut sub = g.subtree("a");
        sub.sort();
        assert_eq!(sub, ["a", "b"]);
    }

    #[test]
    fn a_parent_pointing_nowhere_ends_the_spine() {
        let g = graph(&["orphan:nowhere"]);
        assert!(g.ancestors_of("orphan").is_empty());
    }
}
