//! Demand: effective blocking, reversed — and the ranking built on it.
//!
//! The question this answers is "who is waiting on this?", so that a task standing between
//! somebody and an urgent issue outranks one that blocks nothing. Everything here derives from
//! [`Graph::demand_edges`], which is [`Graph::is_blocked`] turned around.

use super::{Graph, priority_rank};
use crate::config::{self, is_terminal};
use crate::issue::Issue;
use std::collections::{BTreeMap, BTreeSet};

/// `id -> the ids directly waiting on it`, over non-terminal issues only.
type Reverse = BTreeMap<String, BTreeSet<String>>;

impl Graph {
    /// Two channels feed this. An authored edge lifts to `subtree(source)` waiting on
    /// `subtree(target)` — the same relation [`Graph::is_blocked`] reads, turned around. And a
    /// node is demanded by its parent, which is not done until its children are; that alone
    /// lets an urgent epic rank its own leaves.
    ///
    /// Terminal issues drop out of both ends, so they neither count nor conduct: an urgent
    /// dependent closed as `wontfix` stops making its blockers urgent, exactly as it stops
    /// blocking them.
    fn demand_edges(&self) -> Reverse {
        let mut rev = Reverse::new();
        self.add_containment_demand(&mut rev);
        self.add_dependency_demand(&mut rev);
        rev
    }

    /// A node is demanded by the parent that contains it.
    fn add_containment_demand(&self, rev: &mut Reverse) {
        for r in &self.rows {
            if !is_terminal(&r.status)
                && let Some(p) = &r.parent
                && self.get(p).is_some()
                && !self.is_terminal_id(p)
            {
                rev.entry(r.id.clone()).or_default().insert(p.clone());
            }
        }
    }

    /// An authored edge, lifted to whole subtrees on both ends.
    fn add_dependency_demand(&self, rev: &mut Reverse) {
        for a in &self.rows {
            let srcs = self.live_subtree(&a.id);
            if srcs.is_empty() {
                continue;
            }
            for b in self.requires_of(&a.id) {
                for t in self.live_subtree(&b) {
                    rev.entry(t).or_default().extend(srcs.iter().cloned());
                }
            }
        }
    }

    /// `id`'s subtree with the terminal issues dropped — they neither count nor conduct.
    fn live_subtree(&self, id: &str) -> BTreeSet<String> {
        self.subtree(id).into_iter().filter(|n| !self.is_terminal_id(n)).collect()
    }

    /// `id` plus every non-terminal issue transitively waiting on it. An issue nobody
    /// waits on is still in its own cone, so it ranks by its own priority.
    pub(crate) fn demand_cone(&self, id: &str) -> BTreeSet<String> {
        Graph::cone_of(&self.demand_edges(), id)
    }

    fn cone_of(rev: &Reverse, id: &str) -> BTreeSet<String> {
        let mut cone: BTreeSet<String> = BTreeSet::from([id.to_string()]);
        let mut stack = vec![id.to_string()];
        while let Some(n) = stack.pop() {
            for next in rev.get(&n).into_iter().flatten() {
                if cone.insert(next.clone()) {
                    stack.push(next.clone());
                }
            }
        }
        cone
    }

    /// The cone's population per priority, highest first, with a trailing bucket for
    /// unrecognised ones.
    ///
    /// Compared lexicographically this *is* the ranking rule: the first non-zero slot is
    /// the cone's maximum priority (blocking an urgent issue beats being high), and
    /// within a slot a larger count wins (blocking two high issues beats blocking one).
    /// Levels never trade, so no pile of mediums adds up to a high.
    pub(crate) fn demand_vector(&self, id: &str) -> Vec<usize> {
        self.vector_of(&self.demand_edges(), id)
    }

    fn vector_of(&self, rev: &Reverse, id: &str) -> Vec<usize> {
        let mut counts = vec![0usize; config::PRIORITIES.len() + 1];
        for member in Graph::cone_of(rev, id) {
            if let Some(r) = self.get(&member) {
                counts[priority_rank(&r.priority)] += 1;
            }
        }
        counts
    }

    /// The cone member that makes an issue rank above its own priority — the
    /// highest-priority issue waiting on it, or `None` when it is already the maximum.
    /// Ties go to the lowest id, so the note a row carries is stable across runs.
    pub(crate) fn demand_source(&self, id: &str) -> Option<String> {
        let own = priority_rank(self.get(id).map_or("", |r| r.priority.as_str()));
        let mut best: Option<(usize, String)> = None;
        for member in self.demand_cone(id) {
            if member == id {
                continue;
            }
            let rank = priority_rank(self.get(&member).map_or("", |r| r.priority.as_str()));
            if rank < own && best.as_ref().is_none_or(|(b, _)| rank < *b) {
                best = Some((rank, member));
            }
        }
        best.map(|(_, id)| id)
    }

    /// Ready leaves, in the order `ready` and `next` present them: by demand cone, then
    /// `-points`, then id. With no dependencies and no parents every cone is a singleton,
    /// which is the declared-priority sort exactly.
    ///
    /// The edge map is built once and threaded through the comparator; deriving it per
    /// comparison would rebuild the whole reversed graph for every pair.
    pub(crate) fn ranked_ready(&self) -> Vec<String> {
        let rev = self.demand_edges();
        let mut out: Vec<&Issue> = self.rows.iter().filter(|r| self.is_ready(&r.id)).collect();
        out.sort_by(|a, b| self.vector_of(&rev, &a.id).cmp(&self.vector_of(&rev, &b.id)).reverse().then(b.points.cmp(&a.points)).then(a.id.cmp(&b.id)));
        out.into_iter().map(|r| r.id.clone()).collect()
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
    fn demand_lifts_a_blocker_above_its_own_priority() {
        // The rule the whole ranking exists for: a medium task standing between you and
        // an urgent one outranks a high one that blocks nothing.
        let g = graph(&["blocker !medium", "urgent ->blocker !urgent", "high !high"]);
        assert_eq!(g.ranked_ready(), ["blocker", "high"]);
        assert_eq!(g.demand_source("blocker").as_deref(), Some("urgent"));
        assert_eq!(g.demand_source("high"), None, "already its own maximum");
    }

    #[test]
    fn a_started_issue_still_blocks_and_still_conducts_demand() {
        // The narrowing is about what is *offered*, and must not leak into the two things
        // an unfinished issue does regardless of who holds it.
        let g = graph(&["blocker !medium", "urgent ->blocker !urgent @in-progress", "high !high"]);
        assert!(g.is_blocked("urgent"), "a dependency is satisfied by done, not by started");
        assert_eq!(g.ranked_ready(), ["blocker", "high"]);
        assert_eq!(g.demand_source("blocker").as_deref(), Some("urgent"));
    }

    #[test]
    fn within_a_priority_blocking_more_wins() {
        let g = graph(&["one !low", "two !low", "h1 ->two !high", "h2 ->two !high", "h3 ->one !high"]);
        let ranked = g.ranked_ready();
        assert!(ranked.iter().position(|x| x == "two") < ranked.iter().position(|x| x == "one"), "{ranked:?}");
    }

    #[test]
    fn levels_never_trade() {
        // No pile of mediums adds up to a high.
        let g = graph(&["few !lowest", "many !lowest", "h ->few !high", "m1 ->many !medium", "m2 ->many !medium", "m3 ->many !medium"]);
        let ranked = g.ranked_ready();
        assert!(ranked.iter().position(|x| x == "few") < ranked.iter().position(|x| x == "many"), "{ranked:?}");
    }

    #[test]
    fn a_terminal_dependent_neither_counts_nor_conducts() {
        // An urgent issue closed as wontfix stops making its blockers urgent, exactly as
        // it stops blocking them.
        let g = graph(&["blocker !low", "urgent ->blocker !urgent @done", "other !medium"]);
        assert_eq!(g.demand_source("blocker"), None);
        assert_eq!(g.ranked_ready(), ["other", "blocker"]);
    }

    #[test]
    fn an_epic_ranks_its_own_leaves() {
        // A node is demanded by its parent, which is not done until its children are.
        let g = graph(&["epic !urgent", "kid:epic !low", "loose !medium"]);
        assert_eq!(g.ranked_ready(), ["kid", "loose"]);
        assert_eq!(g.demand_source("kid").as_deref(), Some("epic"));
    }

    #[test]
    fn an_issue_nobody_waits_on_still_ranks_by_its_own_priority() {
        let g = graph(&["a !low", "b !urgent", "c !medium"]);
        assert_eq!(g.ranked_ready(), ["b", "c", "a"]);
    }

    #[test]
    fn ties_break_by_points_then_id() {
        let g = graph(&["b !medium #3", "a !medium #3", "c !medium #9"]);
        assert_eq!(g.ranked_ready(), ["c", "a", "b"]);
    }

    /// Demand is transitive: an urgent issue two hops away still lifts the blocker at the
    /// bottom, which is what makes the cone a cone rather than a neighbour list.
    #[test]
    fn demand_travels_the_whole_chain() {
        let g = graph(&["bottom !lowest", "mid ->bottom !lowest", "top ->mid !urgent"]);
        assert!(g.demand_cone("bottom").contains("top"), "{:?}", g.demand_cone("bottom"));
        assert_eq!(g.demand_source("bottom").as_deref(), Some("top"));
    }

    /// A dependency cycle is malformed data that `check` reports — but the cone walk must
    /// terminate before the user gets there.
    #[test]
    fn a_dependency_cycle_does_not_hang_the_cone() {
        let g = graph(&["a ->b", "b ->a"]);
        assert_eq!(g.demand_cone("a").len(), 2);
    }
}
