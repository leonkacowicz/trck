//! The derived view over a loaded index: hierarchy, dependencies that climb it, cycle
//! detection, readiness, and the demand cone the ranking is built on.
//!
//! Nothing here is stored. `index.jsonl` holds declared facts — who is whose child, what
//! depends on what, what status something is in — and everything in this module is
//! recomputed from them on every run. That is deliberate: readiness changes when an
//! *unrelated* issue closes, so as stored state it would be a cascade write on every
//! close and silent corruption on every missed one.
//!
//! The one rule worth reading before the rest is **lifting**. An authored edge `a -> b`
//! is inherited by everything inside `a` and satisfied only by everything inside `b`:
//!
//! * source side — a parent's dependency binds every descendant, so a child cannot be
//!   picked up while its epic is waiting on something.
//! * target side — depending on a parent waits for its whole subtree, because a parent
//!   is terminal only when its children are.
//!
//! Every derived answer here is that rule read in one direction or the other:
//! [`Graph::is_blocked`] reads the source side, [`Graph::demand_edges`] reads it
//! reversed, and the cycle checks in [`cycles`] compose both.

mod cycles;

use crate::config::{self, is_terminal};
use crate::issue::Issue;
use std::collections::{BTreeMap, BTreeSet};

/// Sort key: 0 is the highest priority. Anything unrecognised sorts last — a hand-edited
/// row can still carry junk, and it should sink rather than blow up.
pub(crate) fn priority_rank(priority: &str) -> usize {
    config::PRIORITIES.iter().position(|p| *p == priority).unwrap_or(config::PRIORITIES.len())
}

/// A loaded index plus its derived structure.
pub(crate) struct Graph {
    pub(crate) rows: Vec<Issue>,
    /// id -> position in `rows`.
    index: BTreeMap<String, usize>,
    /// parent id -> child ids, id-sorted.
    children: BTreeMap<String, Vec<String>>,
    /// dependency id -> the ids that authored an edge to it, id-sorted.
    dependents: BTreeMap<String, Vec<String>>,
}

impl Graph {
    pub(crate) fn new(rows: Vec<Issue>) -> Graph {
        let index: BTreeMap<String, usize> = rows.iter().enumerate().map(|(i, r)| (r.id.clone(), i)).collect();
        let mut children: BTreeMap<String, Vec<String>> = BTreeMap::new();
        let mut dependents: BTreeMap<String, Vec<String>> = BTreeMap::new();
        for r in &rows {
            if let Some(p) = &r.parent {
                children.entry(p.clone()).or_default().push(r.id.clone());
            }
            for d in &r.depends_on {
                dependents.entry(d.clone()).or_default().push(r.id.clone());
            }
        }
        for v in children.values_mut().chain(dependents.values_mut()) {
            v.sort();
            v.dedup();
        }
        Graph { rows, index, children, dependents }
    }

    pub(crate) fn get(&self, id: &str) -> Option<&Issue> {
        self.index.get(id).and_then(|i| self.rows.get(*i))
    }

    fn status_of(&self, id: &str) -> Option<&str> {
        self.get(id).map(|r| r.status.as_str())
    }

    fn is_terminal_id(&self, id: &str) -> bool {
        self.status_of(id).is_some_and(is_terminal)
    }

    // --- hierarchy ---------------------------------------------------------- //

    /// `id`'s children, id-sorted. Containment says *what* composes a parent, not in
    /// what sequence, so there is no other order to preserve.
    pub(crate) fn children_of(&self, id: &str) -> &[String] {
        self.children.get(id).map_or(&[], Vec::as_slice)
    }

    pub(crate) fn is_leaf(&self, id: &str) -> bool {
        !self.children.contains_key(id)
    }

    /// The parent spine above `id`, nearest first. A parent pointing at a missing id
    /// ends the spine, and a parent cycle is broken by the `seen` guard — malformed data
    /// must not make the engine loop.
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

    // --- dependencies ------------------------------------------------------- //

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
    /// ancestor's. The *source* side of the lifting rule, and the one shared primitive
    /// the blocking, ranking and cycle checks all read.
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

    // --- predicates --------------------------------------------------------- //

    /// One-sided effective blocking: blocked iff this issue, or any ancestor, has an
    /// authored dependency on a non-terminal issue.
    ///
    /// The depended-on side needs no expansion. A parent is terminal only when its whole
    /// subtree is, so "wait for b" already means "wait for everything inside b".
    pub(crate) fn is_blocked(&self, id: &str) -> bool {
        self.lifted_deps(id).iter().any(|b| !self.is_terminal_id(b))
    }

    /// An unblocked leaf that could be picked up right now.
    ///
    /// `in-review` fails this without being terminal: an issue awaiting judgement is in
    /// flight, not available, so there is nothing to start — but it still blocks whatever
    /// waits on it.
    pub(crate) fn is_ready(&self, id: &str) -> bool {
        let Some(r) = self.get(id) else { return false };
        !is_terminal(&r.status) && config::is_actionable(&r.status) && self.is_leaf(id) && !self.is_blocked(id)
    }

    // --- demand: effective blocking, reversed -------------------------------- //

    /// `id -> the ids directly waiting on it`, over non-terminal issues only.
    ///
    /// Two channels feed it. An authored edge lifts to `subtree(source)` waiting on
    /// `subtree(target)` — the same relation [`Graph::is_blocked`] reads, turned around.
    /// And a node is demanded by its parent, which is not done until its children are;
    /// that alone lets an urgent epic rank its own leaves.
    ///
    /// Terminal issues drop out of both ends, so they neither count nor conduct: an
    /// urgent dependent closed as `wontfix` stops making its blockers urgent, exactly as
    /// it stops blocking them.
    fn demand_edges(&self) -> BTreeMap<String, BTreeSet<String>> {
        let mut rev: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for r in &self.rows {
            if is_terminal(&r.status) {
                continue;
            }
            if let Some(p) = &r.parent
                && self.get(p).is_some()
                && !self.is_terminal_id(p)
            {
                rev.entry(r.id.clone()).or_default().insert(p.clone());
            }
        }
        for a in &self.rows {
            let srcs: BTreeSet<String> = self.subtree(&a.id).into_iter().filter(|n| !self.is_terminal_id(n)).collect();
            if srcs.is_empty() {
                continue;
            }
            for b in self.requires_of(&a.id) {
                for t in self.subtree(&b) {
                    if !self.is_terminal_id(&t) {
                        rev.entry(t).or_default().extend(srcs.iter().cloned());
                    }
                }
            }
        }
        rev
    }

    /// `id` plus every non-terminal issue transitively waiting on it. An issue nobody
    /// waits on is still in its own cone, so it ranks by its own priority.
    pub(crate) fn demand_cone(&self, id: &str) -> BTreeSet<String> {
        Graph::demand_cone_with(&self.demand_edges(), id)
    }

    fn demand_cone_with(rev: &BTreeMap<String, BTreeSet<String>>, id: &str) -> BTreeSet<String> {
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
        self.demand_vector_with(&self.demand_edges(), id)
    }

    fn demand_vector_with(&self, rev: &BTreeMap<String, BTreeSet<String>>, id: &str) -> Vec<usize> {
        let mut counts = vec![0usize; config::PRIORITIES.len() + 1];
        for member in Graph::demand_cone_with(rev, id) {
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
    pub(crate) fn ranked_ready(&self) -> Vec<String> {
        let rev = self.demand_edges();
        let mut out: Vec<&Issue> = self.rows.iter().filter(|r| self.is_ready(&r.id)).collect();
        out.sort_by(|a, b| {
            self.demand_vector_with(&rev, &a.id).cmp(&self.demand_vector_with(&rev, &b.id)).reverse().then(b.points.cmp(&a.points)).then(a.id.cmp(&b.id))
        });
        out.into_iter().map(|r| r.id.clone()).collect()
    }

    /// The ids in an issue's directed dependency line: itself, plus — when `up` —
    /// everything it transitively depends on, and — when `down` — everything that
    /// transitively depends on it.
    ///
    /// Excludes "cousins" joined only through a shared neighbour: unlike a weakly
    /// connected component, the two sweeps never cross direction. Containment is
    /// followed too, so `up` from a parent descends its whole subtree (what it is
    /// waiting on) and `down` from a child climbs to the parents that contain it.
    /// Siblings stay cousins — they meet only at the parent, and neither sweep turns
    /// around there.
    pub(crate) fn dependency_line(&self, id: &str, up: bool, down: bool) -> BTreeSet<String> {
        let mut seen: BTreeSet<String> = BTreeSet::from([id.to_string()]);
        if up {
            let mut stack = vec![id.to_string()];
            while let Some(node) = stack.pop() {
                let mut targets = self.requires_of(&node);
                targets.extend(self.children_of(&node).iter().cloned());
                targets.extend(self.lifted_deps(&node));
                for t in targets {
                    if self.get(&t).is_some() && seen.insert(t.clone()) {
                        stack.push(t);
                    }
                }
            }
        }
        if down {
            let mut stack = vec![id.to_string()];
            while let Some(node) = stack.pop() {
                let mut sources: Vec<String> = self.dependents_of(&node).to_vec();
                if let Some(p) = self.get(&node).and_then(|r| r.parent.clone())
                    && self.get(&p).is_some()
                {
                    sources.push(p);
                }
                for s in sources {
                    if seen.insert(s.clone()) {
                        stack.push(s);
                    }
                }
            }
        }
        seen
    }

    // --- rollup -------------------------------------------------------------- //

    /// `(done_points, total_points, done_count, total_count)` over the leaf descendants.
    ///
    /// A leaf's weight is its own points; a parent's is the sum of its leaves, so points
    /// set on a parent are ignored rather than double-counted. Cycle-guarded, because a
    /// mid-edit index must not spin forever.
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

    use super::*;
    use crate::test_graph::graph;
    use std::fmt::Write as _;

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
    fn depending_on_a_parent_waits_for_its_whole_subtree() {
        // Target side. The depended-on side needs no expansion because a parent is
        // terminal only when its children are — so this is really a statement about
        // rollup, checked here through blocking.
        let g = graph(&["epic", "kid:epic @backlog", "waiting ->epic"]);
        assert!(g.is_blocked("waiting"));
        let g = graph(&["epic @done", "kid:epic @done", "waiting ->epic"]);
        assert!(!g.is_blocked("waiting"));
    }

    #[test]
    fn readiness_is_leaf_only_unblocked_and_actionable() {
        let g = graph(&["epic", "kid:epic", "blocked ->kid", "reviewing @in-review", "finished @done", "free"]);
        assert!(g.is_ready("kid"));
        assert!(!g.is_ready("epic"), "a parent is not something you pick up");
        assert!(!g.is_ready("blocked"));
        assert!(!g.is_ready("reviewing"), "in flight, but its output is pending someone else's judgement");
        assert!(!g.is_ready("finished"));
        assert!(g.is_ready("free"));
    }

    #[test]
    fn a_terminal_blocker_stops_blocking() {
        let g = graph(&["dep @done", "work ->dep"]);
        assert!(!g.is_blocked("work"));
        assert!(g.is_ready("work"));
    }

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

    #[test]
    fn an_unrecognised_priority_sinks_rather_than_blowing_up() {
        assert_eq!(priority_rank("nonesuch"), config::PRIORITIES.len());
        let g = graph(&["junk !nonesuch", "ok !lowest"]);
        assert_eq!(g.ranked_ready(), ["ok", "junk"]);
    }

    /// Answer every derived question over this repo's real 195-issue graph and dump it,
    /// so the Python engine's answers can be diffed against it.
    ///
    /// The unit tests above cover shapes someone thought of. This covers a graph with
    /// real epics, real inherited edges and a real ranking — the place where a subtly
    /// wrong lifting rule shows up and a hand-written fixture would not.
    #[test]
    fn dump_real_graph_answers_for_differential_comparison() {
        let Ok(want) = std::env::var("TRCK_DUMP_GRAPH") else {
            return; // opt-in: this is a comparison harness, not an assertion
        };
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let text = std::fs::read_to_string(root.join("issues/index.jsonl")).expect("index");
        let g = Graph::new(crate::index::parse_index(&text, "index.jsonl").expect("parses"));
        let mut out = String::new();
        let mut ids: Vec<&str> = g.rows.iter().map(|r| r.id.as_str()).collect();
        ids.sort_unstable();
        for id in &ids {
            let pct = g.progress_pct(id).map_or(String::from("-"), |p| p.to_string());
            let _ = writeln!(
                out,
                "{id} leaf={} blocked={} ready={} pct={pct} lifted={} cone={} src={}",
                g.is_leaf(id),
                g.is_blocked(id),
                g.is_ready(id),
                g.lifted_deps(id).join(","),
                g.demand_cone(id).len(),
                g.demand_source(id).unwrap_or_default(),
            );
        }
        let _ = writeln!(out, "ranked={}", g.ranked_ready().join(","));
        std::fs::write(&want, out).expect("write dump");
    }

    #[test]
    fn a_dangling_dependency_is_skipped_not_fatal() {
        let g = graph(&["a ->nowhere"]);
        assert!(g.requires_of("a").is_empty());
        assert!(!g.is_blocked("a"));
    }
}
