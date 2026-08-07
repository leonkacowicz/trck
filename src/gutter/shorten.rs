//! Sliding single nodes along the order to shorten the gutter, keeping it a linear
//! extension.
//!
//! The cost is total lane length: a lane opens at its prerequisite's row and closes at its
//! dependent's, so the sum of those spans is exactly what fills the gutter with idle `│`.
//! Prerequisites-first alone does not care — it will happily emit a root whose only
//! dependent is at the bottom, leaving its lane open the whole way down.
//!
//! Gathered per node rather than per edge, the cost collapses to
//! `sum(pos[v] * (indeg[v] - outdeg[v]))` — linear in the positions. That is what makes
//! this cheap: moving one node shifts a contiguous block by exactly one row, so a
//! candidate's delta reads off a prefix sum in constant time instead of costing a walk over
//! the edges.

use std::collections::{BTreeMap, BTreeSet};

/// The cost model plus where the order currently stands.
///
/// Owned keys throughout: the search mutates the order it is given, so borrowing its
/// strings would pin it for the whole run.
struct Search {
    /// Per node, `indeg - outdeg` — its whole contribution to the cost, times its row.
    weight: BTreeMap<String, i64>,
    /// Each node's prerequisites. Nothing may slide above them.
    after: BTreeMap<String, BTreeSet<String>>,
    /// Each node's dependents. Nothing may slide below them.
    before: BTreeMap<String, BTreeSet<String>>,
    /// Where each node sits now.
    at: BTreeMap<String, usize>,
    /// Running weight total up to each slot, so a moved block's weight is one subtraction.
    prefix: Vec<i64>,
    n: usize,
}

impl Search {
    fn new(order: &[String], pairs: &[(String, String)]) -> Search {
        let mut weight: BTreeMap<String, i64> = order.iter().map(|v| (v.clone(), 0)).collect();
        let mut after: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        let mut before: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for (a, b) in pairs {
            *weight.entry(b.clone()).or_default() += 1;
            *weight.entry(a.clone()).or_default() -= 1;
            after.entry(b.clone()).or_default().insert(a.clone());
            before.entry(a.clone()).or_default().insert(b.clone());
        }
        let mut search = Search { weight, after, before, at: BTreeMap::new(), prefix: Vec::new(), n: order.len() };
        search.resync(order);
        search
    }

    /// Re-read the positions and prefix sums after the order changed.
    fn resync(&mut self, order: &[String]) {
        self.at = order.iter().enumerate().map(|(k, v)| (v.clone(), k)).collect();
        self.prefix = vec![0i64; order.len() + 1];
        let mut run = 0;
        for (k, v) in order.iter().enumerate() {
            run += self.weight.get(v).copied().unwrap_or(0);
            self.prefix[k + 1] = run;
        }
    }

    /// The slots `u` may take with the order still a linear extension: between its last
    /// prerequisite and its first dependent. Every slot in that window keeps it one, and no
    /// slot outside it does.
    fn window(&self, u: &str) -> (usize, usize) {
        let lo = self.after.get(u).into_iter().flatten().filter_map(|p| self.at.get(p)).max().map_or(0, |m| m + 1);
        let hi = self.before.get(u).into_iter().flatten().filter_map(|d| self.at.get(d)).min().map_or(self.n, |m| *m).saturating_sub(1);
        (lo, hi.min(self.n.saturating_sub(1)))
    }

    /// What moving the node at `i` to slot `j` costs. It travels `j - i` rows; everything it
    /// steps over shifts one row the other way, and the prefix sum totals that block's
    /// weight at once. Negative is an improvement.
    fn delta(&self, i: usize, j: usize, w: i64) -> i64 {
        let span = i64::try_from(j).unwrap_or(0) - i64::try_from(i).unwrap_or(0);
        if j > i { w * span - (self.prefix[j + 1] - self.prefix[i + 1]) } else { w * span + (self.prefix[i] - self.prefix[j]) }
    }

    /// The first slot that shortens the gutter for the node at `i`, if any does.
    fn improvement(&self, order: &[String], i: usize) -> Option<usize> {
        let u = order.get(i)?;
        let w = self.weight.get(u).copied().unwrap_or(0);
        let (lo, hi) = self.window(u);
        (lo..=hi).find(|&j| j != i && self.delta(i, j, w) < 0)
    }
}

/// First improvement, repeated until nothing helps. Termination needs no iteration cap: the
/// cost is a non-negative integer and every accepted move drops it by at least one.
pub(super) fn shorten_lanes(order: &mut Vec<String>, pairs: &[(String, String)]) {
    let n = order.len();
    let mut search = Search::new(order, pairs);
    loop {
        let mut moved = false;
        // The scan continues from i+1 over the already-updated order rather than restarting
        // on every accepted move. Restarting reaches a different local optimum — same cost
        // function, different fixed point — and this is the one the goldens were written
        // against.
        for i in 0..n {
            if let Some(j) = search.improvement(order, i) {
                let node = order.remove(i);
                order.insert(j, node);
                search.resync(order);
                moved = true;
            }
        }
        if !moved {
            return;
        }
    }
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    fn ids(names: &[&str]) -> Vec<String> {
        names.iter().map(|s| (*s).to_string()).collect()
    }

    fn edges(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(x, y)| ((*x).to_string(), (*y).to_string())).collect()
    }

    #[test]
    fn a_lone_blocker_slides_down_to_shorten_its_lane() {
        // `a` blocks only `d`; `b -> c -> d` is a chain. Prerequisites-first alone puts
        // `a` first (lowest id among the roots), leaving its lane open beside the whole
        // chain. Nothing forces that: it need only precede `d`.
        let mut order = ids(&["a", "b", "c", "d"]);
        shorten_lanes(&mut order, &edges(&[("a", "d"), ("b", "c"), ("c", "d")]));
        assert_eq!(order, ids(&["b", "c", "a", "d"]));
    }

    #[test]
    fn shortening_never_breaks_prerequisites_first() {
        let mut order = ids(&["a", "b", "c", "d", "e", "f"]);
        let pairs = edges(&[("a", "d"), ("b", "c"), ("c", "d"), ("a", "e"), ("d", "f"), ("e", "f")]);
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
        let pairs = edges(&[("a", "d"), ("b", "c"), ("c", "d")]);
        let mut first = ids(&["a", "b", "c", "d"]);
        let mut second = ids(&["b", "a", "c", "d"]);
        shorten_lanes(&mut first, &pairs);
        shorten_lanes(&mut second, &pairs);
        assert_eq!(first, second);
    }

    #[test]
    fn a_chain_is_already_as_short_as_it_gets() {
        // Every lane in a chain spans one row, so the cost is at its floor and no slide can
        // beat it — the fixed point has to be the order it arrived in.
        let mut order = ids(&["a", "b", "c", "d", "e"]);
        shorten_lanes(&mut order, &edges(&[("a", "b"), ("b", "c"), ("c", "d"), ("d", "e")]));
        assert_eq!(order, ids(&["a", "b", "c", "d", "e"]));
    }

    #[test]
    fn an_edgeless_order_is_left_alone() {
        let mut order = ids(&["c", "a", "b"]);
        shorten_lanes(&mut order, &[]);
        assert_eq!(order, ids(&["c", "a", "b"]));
    }
}
