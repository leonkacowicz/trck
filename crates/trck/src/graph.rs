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
//! reversed, and the cycle checks compose both.

use crate::config::{self, is_terminal};
use crate::issue::Issue;
use std::collections::{BTreeMap, BTreeSet};

/// Sort key: 0 is the highest priority. Anything unrecognised sorts last — a hand-edited
/// row can still carry junk, and it should sink rather than blow up.
pub(crate) fn priority_rank(priority: &str) -> usize {
    config::PRIORITIES
        .iter()
        .position(|p| *p == priority)
        .unwrap_or(config::PRIORITIES.len())
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
        let index: BTreeMap<String, usize> = rows
            .iter()
            .enumerate()
            .map(|(i, r)| (r.id.clone(), i))
            .collect();
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
        Graph {
            rows,
            index,
            children,
            dependents,
        }
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
        let mut out: Vec<String> = self.get(id).map_or_else(Vec::new, |r| {
            r.depends_on
                .iter()
                .filter(|d| self.get(d).is_some())
                .cloned()
                .collect()
        });
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
        !is_terminal(&r.status)
            && config::is_actionable(&r.status)
            && self.is_leaf(id)
            && !self.is_blocked(id)
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
            let srcs: BTreeSet<String> = self
                .subtree(&a.id)
                .into_iter()
                .filter(|n| !self.is_terminal_id(n))
                .collect();
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
            self.demand_vector_with(&rev, &a.id)
                .cmp(&self.demand_vector_with(&rev, &b.id))
                .reverse()
                .then(b.points.cmp(&a.points))
                .then(a.id.cmp(&b.id))
        });
        out.into_iter().map(|r| r.id.clone()).collect()
    }

    // --- cycles -------------------------------------------------------------- //

    /// Everything effectively depended on, transitively, by anything in `start`: from
    /// each node climb the spine to inherit authored deps, then expand each target's
    /// subtree. Restarting from every target is what chains hops correctly.
    fn effective_reach(&self, start: impl IntoIterator<Item = String>) -> BTreeSet<String> {
        let mut seen: BTreeSet<String> = BTreeSet::new();
        let mut stack: Vec<String> = start.into_iter().collect();
        while let Some(node) = stack.pop() {
            for b in self.lifted_deps(&node) {
                for t in self.subtree(&b) {
                    if seen.insert(t.clone()) {
                        stack.push(t);
                    }
                }
            }
        }
        seen
    }

    /// The parent-spine relationship between two issues, or `None` when their subtrees
    /// are disjoint: `"same"`, `"descendant"` (a is below b), `"ancestor"` (a is above b).
    ///
    /// A dependency edge is admissible only when this is `None`. Overlapping subtrees
    /// self-cycle under lifting — but *not* in a way `would_cycle` detects, because with
    /// no authored edges anywhere there is nothing for it to reach through. It is a
    /// separate invariant, checked separately, and it is cheap: one spine walk.
    pub(crate) fn containment(&self, a: &str, b: &str) -> Option<&'static str> {
        if a == b {
            return Some("same");
        }
        if self.get(a).is_none() || self.get(b).is_none() {
            return None;
        }
        if self.ancestors_of(a).iter().any(|x| x == b) {
            return Some("descendant");
        }
        if self.ancestors_of(b).iter().any(|x| x == a) {
            return Some("ancestor");
        }
        None
    }

    /// Why a candidate `src depends_on dep` edge must be refused, or `None` when it is
    /// admissible. Checked before anything is written, so a rejection persists nothing.
    pub(crate) fn check_dep_edge(&self, src: &str, dep: &str) -> Option<String> {
        match self.containment(src, dep) {
            Some("same") => return Some(format!("#{src} cannot depend on itself")),
            Some("descendant") => {
                return Some(format!(
                    "#{src} is a descendant of #{dep}; a node can't depend on its own \
                     ancestor (their subtrees overlap)"
                ));
            }
            Some("ancestor") => {
                return Some(format!(
                    "#{src} is an ancestor of #{dep}; a node can't depend on its own \
                     descendant (their subtrees overlap)"
                ));
            }
            _ => {}
        }
        if self.would_cycle(src, dep) {
            return Some(format!(
                "#{src} -> #{dep} would create an effective dependency cycle \
                 (a dependency inherited through the parent hierarchy closes the loop)"
            ));
        }
        None
    }

    /// Whether adding `src depends_on dep` would close an *effective* cycle — one
    /// implied through the hierarchy, not only through authored edges.
    ///
    /// The new edge lifts to `subtree(src)` waiting on `subtree(dep)`, so it closes a
    /// loop exactly when something inside `dep` already effectively reaches something
    /// inside `src`. An issue and its own ancestor or descendant can never depend on each
    /// other, which falls out of this rather than being a separate rule.
    pub(crate) fn would_cycle(&self, src: &str, dep: &str) -> bool {
        if src == dep {
            return true;
        }
        if self.get(src).is_none() || self.get(dep).is_none() {
            return false;
        }
        let src_ids: BTreeSet<String> = self.subtree(src).into_iter().collect();
        let reached = self.effective_reach(self.subtree(dep));
        reached.intersection(&src_ids).next().is_some()
    }

    /// Every cycle in the effective dependency graph, each reported once, as the ids in
    /// loop order. A superset of the authored cycles: an authored cycle is an effective
    /// one too.
    pub(crate) fn effective_cycles(&self) -> Vec<Vec<String>> {
        let mut succ: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for x in &self.rows {
            let entry = succ.entry(x.id.clone()).or_default();
            for b in self.lifted_deps(&x.id) {
                entry.extend(self.subtree(&b));
            }
        }
        find_cycles(&succ)
    }

    /// Cycles in the parent hierarchy, which make the tree not a tree. Reported
    /// separately because the fix is different: a dependency cycle is an authoring
    /// mistake, a parent cycle is structural damage.
    pub(crate) fn parent_cycles(&self) -> Vec<Vec<String>> {
        let mut succ: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
        for r in &self.rows {
            let entry = succ.entry(r.id.clone()).or_default();
            if let Some(p) = &r.parent
                && self.get(p).is_some()
            {
                entry.insert(p.clone());
            }
        }
        find_cycles(&succ)
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
            return (
                if done { r.points } else { 0 },
                r.points,
                usize::from(done),
                1,
            );
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

/// Every cycle in a successor map, each reported once, in id order.
fn find_cycles(succ: &BTreeMap<String, BTreeSet<String>>) -> Vec<Vec<String>> {
    let mut cycles: Vec<Vec<String>> = Vec::new();
    let mut seen_keys: BTreeSet<Vec<String>> = BTreeSet::new();
    let mut colour: BTreeMap<&str, u8> = BTreeMap::new(); // 0 unseen, 1 on stack, 2 done
    let mut path: Vec<String> = Vec::new();

    // Iterative depth-first search. Recursion would be shorter and would also blow the
    // stack on a deep hierarchy, which a malformed index can produce.
    for start in succ.keys() {
        if colour.get(start.as_str()).copied().unwrap_or(0) != 0 {
            continue;
        }
        let mut stack: Vec<(&str, Vec<String>)> = vec![(
            start.as_str(),
            succ.get(start)
                .map(|s| s.iter().cloned().collect())
                .unwrap_or_default(),
        )];
        colour.insert(start.as_str(), 1);
        path.push(start.clone());
        while let Some((node, kids)) = stack.last_mut() {
            let Some(next) = kids.pop() else {
                colour.insert(node, 2);
                path.pop();
                stack.pop();
                continue;
            };
            match colour.get(next.as_str()).copied().unwrap_or(0) {
                1 => {
                    // Back edge: the loop is the path from `next` to the top.
                    if let Some(at) = path.iter().position(|n| *n == next) {
                        let cycle = path[at..].to_vec();
                        let mut key = cycle.clone();
                        key.sort();
                        if seen_keys.insert(key) {
                            cycles.push(cycle);
                        }
                    }
                }
                0 => {
                    let Some((k, _)) = succ.get_key_value(&next) else {
                        continue;
                    };
                    colour.insert(k.as_str(), 1);
                    path.push(next.clone());
                    let kids = succ
                        .get(&next)
                        .map(|s| s.iter().cloned().collect())
                        .unwrap_or_default();
                    stack.push((k.as_str(), kids));
                }
                _ => {}
            }
        }
    }
    cycles.sort();
    cycles
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::json::parse;
    use std::fmt::Write as _;

    /// `id[:parent][ ->dep,dep][ @status][ !priority][ #points]`, so a graph reads as
    /// one line per issue instead of six lines of struct literal.
    fn issue(spec: &str) -> Issue {
        let mut parent = String::new();
        let mut deps: Vec<String> = Vec::new();
        let mut status = "backlog".to_string();
        let mut priority = "medium".to_string();
        let mut points = 1i64;
        for part in spec.split_whitespace().skip(1) {
            match part.chars().next() {
                Some('-') => deps = part[2..].split(',').map(str::to_string).collect(),
                Some('@') => status = part[1..].to_string(),
                Some('!') => priority = part[1..].to_string(),
                Some('#') => points = part[1..].parse().unwrap_or(1),
                _ => {}
            }
        }
        let mut id = spec.split_whitespace().next().unwrap_or("x").to_string();
        if let Some((a, b)) = id.clone().split_once(':') {
            id = a.to_string();
            parent = b.to_string();
        }
        let json = format!(
            r#"{{"id": "{id}", "slug": "{id}", "title": "{id}", "status": "{status}",
                 "priority": "{priority}", "points": {points}{}{}}}"#,
            if parent.is_empty() {
                String::new()
            } else {
                format!(r#", "parent": "{parent}""#)
            },
            if deps.is_empty() {
                String::new()
            } else {
                format!(
                    r#", "depends_on": [{}]"#,
                    deps.iter()
                        .map(|d| format!("\"{d}\""))
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            }
        );
        Issue::from_json(&parse(&json).expect("valid json")).expect("valid issue")
    }

    fn graph(specs: &[&str]) -> Graph {
        Graph::new(specs.iter().map(|s| issue(s)).collect())
    }

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
        let g = graph(&[
            "epic",
            "kid:epic",
            "blocked ->kid",
            "reviewing @in-review",
            "finished @done",
            "free",
        ]);
        assert!(g.is_ready("kid"));
        assert!(!g.is_ready("epic"), "a parent is not something you pick up");
        assert!(!g.is_ready("blocked"));
        assert!(
            !g.is_ready("reviewing"),
            "in flight, but its output is pending someone else's judgement"
        );
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
        let g = graph(&[
            "one !low",
            "two !low",
            "h1 ->two !high",
            "h2 ->two !high",
            "h3 ->one !high",
        ]);
        let ranked = g.ranked_ready();
        assert!(
            ranked.iter().position(|x| x == "two") < ranked.iter().position(|x| x == "one"),
            "{ranked:?}"
        );
    }

    #[test]
    fn levels_never_trade() {
        // No pile of mediums adds up to a high.
        let g = graph(&[
            "few !lowest",
            "many !lowest",
            "h ->few !high",
            "m1 ->many !medium",
            "m2 ->many !medium",
            "m3 ->many !medium",
        ]);
        let ranked = g.ranked_ready();
        assert!(
            ranked.iter().position(|x| x == "few") < ranked.iter().position(|x| x == "many"),
            "{ranked:?}"
        );
    }

    #[test]
    fn a_terminal_dependent_neither_counts_nor_conducts() {
        // An urgent issue closed as wontfix stops making its blockers urgent, exactly as
        // it stops blocking them.
        let g = graph(&[
            "blocker !low",
            "urgent ->blocker !urgent @done",
            "other !medium",
        ]);
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
    fn a_direct_cycle_is_refused() {
        let g = graph(&["a", "b ->a"]);
        assert!(g.would_cycle("a", "b"));
        assert!(
            g.check_dep_edge("a", "b")
                .is_some_and(|m| m.contains("cycle"))
        );
    }

    #[test]
    fn an_ancestor_or_descendant_edge_is_refused_by_containment_not_by_reachability() {
        // Worth pinning, because the obvious guess is wrong: with no authored edges
        // anywhere, `would_cycle` has nothing to reach through and says the edge is
        // fine. Overlapping subtrees are a separate invariant.
        let g = graph(&["epic", "kid:epic"]);
        assert!(
            !g.would_cycle("kid", "epic"),
            "reachability cannot see this"
        );
        assert_eq!(g.containment("kid", "epic"), Some("descendant"));
        assert_eq!(g.containment("epic", "kid"), Some("ancestor"));
        assert!(
            g.check_dep_edge("kid", "epic")
                .is_some_and(|m| m.contains("ancestor"))
        );
        assert!(
            g.check_dep_edge("epic", "kid")
                .is_some_and(|m| m.contains("descendant"))
        );
    }

    #[test]
    fn an_issue_cannot_depend_on_itself() {
        let g = graph(&["a"]);
        assert_eq!(g.containment("a", "a"), Some("same"));
        assert_eq!(
            g.check_dep_edge("a", "a").as_deref(),
            Some("#a cannot depend on itself")
        );
    }

    #[test]
    fn siblings_and_cousins_may_depend_on_each_other() {
        let g = graph(&["epic", "one:epic", "two:epic"]);
        assert_eq!(g.containment("two", "one"), None);
        assert!(!g.would_cycle("two", "one"));
        assert_eq!(g.check_dep_edge("two", "one"), None);
    }

    #[test]
    fn a_cycle_implied_through_the_hierarchy_is_refused() {
        // `kid` inherits `epic -> other`, so `other -> kid` closes a loop that neither
        // authored edge shows on its own.
        let g = graph(&["epic ->other", "kid:epic", "other"]);
        assert!(g.would_cycle("other", "kid"));
        assert!(
            g.check_dep_edge("other", "kid")
                .is_some_and(|m| m.contains("inherited"))
        );
        assert!(!g.would_cycle("other", "unrelated"));
    }

    #[test]
    fn effective_cycles_finds_what_would_cycle_would_have_prevented() {
        let g = graph(&["epic ->other", "kid:epic", "other ->kid"]);
        let cycles = g.effective_cycles();
        assert!(!cycles.is_empty(), "an implied loop should be reported");
        let clean = graph(&["a", "b ->a", "c ->b"]);
        assert!(clean.effective_cycles().is_empty());
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
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .parent()
            .and_then(std::path::Path::parent)
            .expect("repo root");
        let text = std::fs::read_to_string(root.join("issues/index.jsonl")).expect("index");
        let g = Graph::new(crate::index::parse_index(&text, "index.jsonl").expect("parses"));
        let mut out = String::new();
        let mut ids: Vec<&str> = g.rows.iter().map(|r| r.id.as_str()).collect();
        ids.sort_unstable();
        for id in &ids {
            let pct = g
                .progress_pct(id)
                .map_or(String::from("-"), |p| p.to_string());
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
