//! Cycle detection over the effective dependency graph, and the edge check that keeps
//! one from ever being written.
//!
//! Split from the rest of [`Graph`] because it reads the lifting rule in a direction
//! nothing else does — *reachability*, restarted from every target so hops chain — and
//! because it is the only part with a job outside the read verbs: `dep` and `set` call
//! [`Graph::check_dep_edge`] before they persist anything, and `check` calls the two
//! `*_cycles` reporters over an index that may already have been hand-edited into one.
//!
//! Two invariants, deliberately separate. An edge into an overlapping subtree is refused
//! by [`Graph::containment`], a spine walk; an edge that closes a loop *through* the
//! hierarchy is refused by [`Graph::would_cycle`], a reachability search. Neither sees
//! what the other catches — see the test that pins exactly that.

use super::Graph;
use std::collections::{BTreeMap, BTreeSet};

impl Graph {
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
            },
            Some("ancestor") => {
                return Some(format!(
                    "#{src} is an ancestor of #{dep}; a node can't depend on its own \
                     descendant (their subtrees overlap)"
                ));
            },
            _ => {},
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
        let mut stack: Vec<(&str, Vec<String>)> = vec![(start.as_str(), succ.get(start).map(|s| s.iter().cloned().collect()).unwrap_or_default())];
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
                },
                0 => {
                    let Some((k, _)) = succ.get_key_value(&next) else {
                        continue;
                    };
                    colour.insert(k.as_str(), 1);
                    path.push(next.clone());
                    let kids = succ.get(&next).map(|s| s.iter().cloned().collect()).unwrap_or_default();
                    stack.push((k.as_str(), kids));
                },
                _ => {},
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

    use crate::test_graph::graph;

    #[test]
    fn a_direct_cycle_is_refused() {
        let g = graph(&["a", "b ->a"]);
        assert!(g.would_cycle("a", "b"));
        assert!(g.check_dep_edge("a", "b").is_some_and(|m| m.contains("cycle")));
    }

    #[test]
    fn an_ancestor_or_descendant_edge_is_refused_by_containment_not_by_reachability() {
        // Worth pinning, because the obvious guess is wrong: with no authored edges
        // anywhere, `would_cycle` has nothing to reach through and says the edge is
        // fine. Overlapping subtrees are a separate invariant.
        let g = graph(&["epic", "kid:epic"]);
        assert!(!g.would_cycle("kid", "epic"), "reachability cannot see this");
        assert_eq!(g.containment("kid", "epic"), Some("descendant"));
        assert_eq!(g.containment("epic", "kid"), Some("ancestor"));
        assert!(g.check_dep_edge("kid", "epic").is_some_and(|m| m.contains("ancestor")));
        assert!(g.check_dep_edge("epic", "kid").is_some_and(|m| m.contains("descendant")));
    }

    #[test]
    fn an_issue_cannot_depend_on_itself() {
        let g = graph(&["a"]);
        assert_eq!(g.containment("a", "a"), Some("same"));
        assert_eq!(g.check_dep_edge("a", "a").as_deref(), Some("#a cannot depend on itself"));
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
        assert!(g.check_dep_edge("other", "kid").is_some_and(|m| m.contains("inherited")));
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
}
