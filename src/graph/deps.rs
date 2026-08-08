//! Dependencies, and the source side of the lifting rule.
//!
//! [`Graph::lifted_deps`] is the shared primitive: the targets visible to an issue through
//! the hierarchy. Blocking, ranking and the cycle checks all read it, which is why it lives
//! here rather than being re-derived by each of them.

use super::Graph;
use std::collections::BTreeSet;

/// Which way [`Graph::sweep`] walks.
///
/// The two directions are the same walk over different neighbours, and the point is that they
/// never cross: a single sweep that followed both would collect the weakly connected
/// component, and cousins with it.
enum Direction {
    /// Toward what an issue is waiting on.
    Up,
    /// Toward what is waiting on it.
    Down,
}

impl Graph {
    /// The authored targets `id` depends on, id-sorted, skipping dangling ids.
    pub(crate) fn requires_of(&self, id: &str) -> Vec<String> {
        let mut out: Vec<String> = self.get(id).map_or_else(Vec::new, |r| r.depends_on.iter().filter(|d| self.get(d).is_some()).cloned().collect());
        out.sort();
        out.dedup();
        out
    }

    pub(crate) fn dependents_of(&self, id: &str) -> &[String] {
        self.dependents.get(id).map_or(&[], Vec::as_slice)
    }

    /// The authored targets visible to `id` through the hierarchy: its own plus every
    /// ancestor's. The *source* side of the lifting rule.
    ///
    /// Nearest author first, id-sorted within an author, and a target reached twice
    /// keeps its nearest author — so an edge an issue authored itself never reads as
    /// inherited.
    pub(crate) fn lifted_deps(&self, id: &str) -> Vec<String> {
        let mut out = Vec::new();
        let mut seen: BTreeSet<String> = BTreeSet::new();
        for author in std::iter::once(id.to_string()).chain(self.ancestors_of(id)) {
            for target in self.requires_of(&author) {
                if seen.insert(target.clone()) {
                    out.push(target);
                }
            }
        }
        out
    }

    /// The ids in an issue's directed dependency line: itself, plus — when `up` —
    /// everything it transitively depends on, and — when `down` — everything that
    /// transitively depends on it.
    ///
    /// Excludes "cousins" joined only through a shared neighbour: unlike a weakly
    /// connected component, the two sweeps never cross direction. Siblings stay cousins —
    /// they meet only at the parent, and neither sweep turns around there.
    pub(crate) fn dependency_line(&self, id: &str, up: bool, down: bool) -> BTreeSet<String> {
        let mut seen: BTreeSet<String> = BTreeSet::from([id.to_string()]);
        if up {
            self.sweep(id, &Direction::Up, &mut seen);
        }
        if down {
            self.sweep(id, &Direction::Down, &mut seen);
        }
        seen
    }

    /// Flood `seen` outward from `id`, following one direction's neighbours only.
    fn sweep(&self, id: &str, dir: &Direction, seen: &mut BTreeSet<String>) {
        let mut stack = vec![id.to_string()];
        while let Some(node) = stack.pop() {
            for next in self.neighbours(&node, dir) {
                if seen.insert(next.clone()) {
                    stack.push(next);
                }
            }
        }
    }

    /// One node's neighbours in one direction, dangling ids dropped.
    ///
    /// Containment is followed as well as dependency, which is what makes `up` from a parent
    /// descend its whole subtree — what it is waiting on — and `down` from a child climb to
    /// the parents that contain it.
    fn neighbours(&self, node: &str, dir: &Direction) -> Vec<String> {
        let mut out = match *dir {
            Direction::Up => {
                let mut targets = self.requires_of(node);
                targets.extend(self.children_of(node).iter().cloned());
                targets.extend(self.lifted_deps(node));
                targets
            },
            Direction::Down => {
                let mut sources: Vec<String> = self.dependents_of(node).to_vec();
                sources.extend(self.get(node).and_then(|r| r.parent.clone()));
                sources
            },
        };
        out.retain(|n| self.get(n).is_some());
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
    fn a_dependency_is_inherited_by_every_descendant() {
        // Source side of the lifting rule: a child cannot be picked up while its epic
        // is waiting on something.
        let g = graph(&["blocker", "epic ->blocker", "kid:epic"]);
        assert_eq!(g.lifted_deps("kid"), ["blocker"]);
        assert!(g.is_blocked("kid"));
        assert!(!g.is_ready("kid"));
    }

    #[test]
    fn a_dangling_dependency_is_skipped_not_fatal() {
        let g = graph(&["a ->nowhere"]);
        assert!(g.requires_of("a").is_empty());
        assert!(!g.is_blocked("a"));
    }

    /// The two sweeps must not meet. Siblings under one parent are joined only *at* the
    /// parent, and a line that turned around there would report every cousin as related —
    /// which is the difference between this and a connected component.
    #[test]
    fn siblings_are_cousins_not_a_dependency_line() {
        let g = graph(&["epic", "a:epic", "b:epic"]);
        let line = g.dependency_line("a", true, true);
        assert!(line.contains("epic"), "the parent contains it: {line:?}");
        assert!(!line.contains("b"), "a sibling is not on the line: {line:?}");
    }

    #[test]
    fn each_direction_reaches_only_its_own_cone() {
        let g = graph(&["dep", "mid ->dep", "top ->mid"]);
        let up = g.dependency_line("mid", true, false);
        assert!(up.contains("dep") && !up.contains("top"), "{up:?}");
        let down = g.dependency_line("mid", false, true);
        assert!(down.contains("top") && !down.contains("dep"), "{down:?}");
        // Both together is the whole chain, and transitively so.
        assert_eq!(g.dependency_line("mid", true, true).len(), 3);
    }
}
