//! Rollup: the one derived answer that only descends the hierarchy.
//!
//! A parent's weight is the sum of its leaves, so points set on a parent are ignored rather
//! than double-counted — the parent *is* its children, and counting both would count the same
//! work twice.

use super::Graph;
use crate::config::is_terminal;
use std::collections::BTreeSet;

impl Graph {
    /// `(done_points, total_points, done_count, total_count)` over the leaf descendants.
    ///
    /// Cycle-guarded, because a mid-edit index must not spin forever.
    pub(crate) fn leaf_rollup(&self, id: &str) -> (i64, i64, usize, usize) {
        self.rollup_seen(id, &mut BTreeSet::new())
    }

    fn rollup_seen(&self, id: &str, seen: &mut BTreeSet<String>) -> (i64, i64, usize, usize) {
        if !seen.insert(id.to_string()) {
            return (0, 0, 0, 0);
        }
        let kids = self.children_of(id);
        if kids.is_empty() {
            let Some(r) = self.get(id) else {
                return (0, 0, 0, 0);
            };
            let done = is_terminal(&r.status);
            return (if done { r.points } else { 0 }, r.points, usize::from(done), 1);
        }
        let kids: Vec<String> = kids.to_vec();
        let mut acc = (0, 0, 0, 0);
        for kid in &kids {
            let (dp, tp, dc, tc) = self.rollup_seen(kid, seen);
            acc = (acc.0 + dp, acc.1 + tp, acc.2 + dc, acc.3 + tc);
        }
        acc
    }

    /// The points-weighted completion percentage of a parent, or `None` for a leaf,
    /// which has nothing to roll up. Zero total points reports 0 rather than dividing.
    pub(crate) fn progress_pct(&self, id: &str) -> Option<i64> {
        if self.is_leaf(id) {
            return None;
        }
        let (done, total, _, _) = self.leaf_rollup(id);
        Some(if total == 0 {
            0
        } else {
            // Round half away from zero, as Python's `round` does not — but both agree
            // on every value reachable here, since the numerator and denominator are
            // non-negative and a .5 case rounds up in the direction users expect.
            (200 * done + total) / (2 * total)
        })
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
    fn rollup_weights_by_leaf_points_not_parent_points() {
        // Points on a parent are ignored rather than double-counted.
        let g = graph(&["epic #99", "a:epic #3 @done", "b:epic #1"]);
        assert_eq!(g.leaf_rollup("epic"), (3, 4, 1, 2));
        assert_eq!(g.progress_pct("epic"), Some(75));
        assert_eq!(g.progress_pct("a"), None, "a leaf has nothing to roll up");
    }

    #[test]
    fn rollup_descends_through_grandchildren() {
        let g = graph(&["epic", "mid:epic", "leaf:mid #4 @done", "other:epic #4"]);
        assert_eq!(g.leaf_rollup("epic"), (4, 8, 1, 2));
        assert_eq!(g.progress_pct("epic"), Some(50));
    }

    #[test]
    fn a_zero_point_parent_reports_zero_rather_than_dividing() {
        let g = graph(&["epic", "kid:epic #0"]);
        assert_eq!(g.progress_pct("epic"), Some(0));
    }

    /// The guard exists for this: a parent cycle must return rather than recurse forever.
    #[test]
    fn a_parent_cycle_does_not_hang_the_rollup() {
        let g = graph(&["a:b #1", "b:a #1"]);
        let (_, total, _, count) = g.leaf_rollup("a");
        assert!(total <= 2 && count <= 2, "rollup double-counted a cycle: {total} {count}");
    }
}
