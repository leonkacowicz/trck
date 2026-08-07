//! The order rows come out in: what `--sort` asks for, and the topological rank the
//! forest projects onto each sibling group before falling back to it.

use crate::config::is_terminal;
use crate::graph::{Graph, priority_rank};
use crate::issue::Issue;
use crate::render::field_value;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet, BinaryHeap};

/// Where each id sits in the order, lowest first.
pub(crate) type Rank = BTreeMap<String, usize>;

/// What `--sort` asks for, as a tuple that compares in the right order.
fn sort_key(r: &Issue, sort: &str) -> (usize, String, String) {
    match sort {
        "priority" => (priority_rank(&r.priority), String::new(), r.id.clone()),
        "points" => (
            // Descending, so a bigger weight sorts first.
            usize::MAX - usize::try_from(r.points.max(0)).unwrap_or(0),
            String::new(),
            r.id.clone(),
        ),
        "id" => (0, r.id.clone(), r.id.clone()),
        _ if sort.starts_with("field:") => {
            let name = &sort["field:".len()..];
            // Rows carrying the field sort by value; rows without it sort last.
            field_value(r, name).map_or_else(|| (1, String::new(), r.id.clone()), |v| (0, v, r.id.clone()))
        },
        _ => (0, r.created.clone().unwrap_or_default(), r.id.clone()),
    }
}

/// The `--sort` key for an id: what seeds the rank, and what breaks a tie in it.
///
/// The fallback is there only so a dangling id has somewhere to sort; every id asked
/// about comes from the graph.
pub(crate) fn seed_key(g: &Graph, id: &str, sort: &str) -> (usize, String, String) {
    g.get(id).map_or_else(|| (0, String::new(), id.to_string()), |r| sort_key(r, sort))
}

/// A rank over every row, to be projected onto each sibling group.
pub(crate) fn sibling_rank<K: Ord>(g: &Graph, key: impl Fn(&str) -> K) -> Rank {
    let succ = constraints(g);
    lift(g, &kahn(g, &succ, &key))
}

/// `id -> the ids that may not come before it`: the **effective** dependencies, expanded
/// on both sides the way the hierarchy expands them.
///
/// - The source side comes from [`Graph::lifted_deps`] rather than the authored list, so
///   an edge authored on a parent holds down every issue beneath it.
/// - The target side expands to the whole subtree, because a parent is done only when its
///   descendants are: waiting on an epic is waiting on everything inside it.
///
/// This is the relation `check` walks for effective cycles, and nothing else. Containment
/// on its own is deliberately **not** an edge: a "child before parent" edge would rank
/// every parent after its whole subtree, and no later adjustment can undo that without
/// also moving epics in a tracker that authored no dependencies at all.
///
/// A dependency that is already **done** constrains nothing, which is the rule
/// [`Graph::is_blocked`] applies and the one that clears a row's `needs #NNN` note. A
/// finished blocker says nothing about the order of the work that is left, and an order
/// disagreeing with the note printed beside it would be worse than no order.
fn constraints(g: &Graph) -> BTreeMap<String, BTreeSet<String>> {
    let mut succ: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for r in &g.rows {
        for d in g.lifted_deps(&r.id) {
            for m in g.subtree(&d).into_iter().filter(|m| g.get(m).is_some_and(|b| !is_terminal(&b.status))) {
                succ.entry(m).or_default().insert(r.id.clone());
            }
        }
    }
    succ
}

/// How many constraints each row is still waiting on.
fn indegrees<'a>(g: &'a Graph, succ: &BTreeMap<String, BTreeSet<String>>) -> BTreeMap<&'a str, usize> {
    let mut indeg: BTreeMap<&str, usize> = g.rows.iter().map(|r| (r.id.as_str(), 0)).collect();
    for t in succ.values().flatten() {
        if let Some(d) = indeg.get_mut(t.as_str()) {
            *d += 1;
        }
    }
    indeg
}

/// Clear `id` from its successors, returning those left waiting on nothing.
fn release(succ: &BTreeMap<String, BTreeSet<String>>, indeg: &mut BTreeMap<&str, usize>, id: &str) -> Vec<String> {
    let mut freed = Vec::new();
    for t in succ.get(id).into_iter().flatten() {
        if let Some(d) = indeg.get_mut(t.as_str())
            && *d > 0
        {
            *d -= 1;
            if *d == 0 {
                freed.push(t.clone());
            }
        }
    }
    freed
}

/// Kahn's algorithm, taking the smallest `key` among the rows that are ready.
///
/// A stack would be cheaper, and is what the gutter uses — depth-first locality is what
/// keeps its lanes short. Here it would let branch structure decide sibling order; the
/// heap lets `--sort` decide it instead, so a tracker whose rows no dependency separates
/// comes out in exactly the order `--sort` asked for.
///
/// Whatever a cycle strands is appended in `key` order rather than dropped: `check` only
/// *warns* about an effective cycle, so `list` still has to render one.
fn kahn<K: Ord>(g: &Graph, succ: &BTreeMap<String, BTreeSet<String>>, key: &impl Fn(&str) -> K) -> Rank {
    let mut indeg = indegrees(g, succ);
    let mut ready: BinaryHeap<Reverse<(K, String)>> = indeg.iter().filter(|(_, d)| **d == 0).map(|(id, _)| Reverse((key(id), (*id).to_string()))).collect();
    let mut rank = Rank::new();
    while let Some(Reverse((_, id))) = ready.pop() {
        rank.insert(id.clone(), rank.len());
        for freed in release(succ, &mut indeg, &id) {
            ready.push(Reverse((key(&freed), freed)));
        }
    }
    let mut stranded: Vec<&str> = g.rows.iter().map(|r| r.id.as_str()).filter(|id| !rank.contains_key(*id)).collect();
    stranded.sort_by_cached_key(|id| (key(id), (*id).to_string()));
    for id in stranded {
        rank.insert(id.to_string(), rank.len());
    }
    rank
}

/// A parent ranks at the earlier of its own row and the start of its work.
///
/// An epic is a container, and the reason to look at one is what is inside it: an epic
/// holding a row that leads the order should lead with it rather than sit wherever
/// `--sort` happened to put the container. Taking the *minimum* and not the subtree's
/// start alone is what keeps an epic where `--sort` put it when nothing has moved its
/// contents — an epic filed months before its first child would otherwise sink to the
/// child, in a tracker with no dependencies at all.
fn lift(g: &Graph, rank: &Rank) -> Rank {
    let mut out = rank.clone();
    for r in &g.rows {
        let Some(own) = rank.get(&r.id) else { continue };
        for a in g.ancestors_of(&r.id) {
            let slot = out.entry(a).or_insert(*own);
            *slot = (*slot).min(*own);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::test_graph::graph;

    /// The ids in rank order, so a test reads as the order it is asserting.
    fn order(g: &Graph) -> Vec<String> {
        let rank = sibling_rank(g, str::to_string);
        let mut ids: Vec<String> = g.rows.iter().map(|r| r.id.clone()).collect();
        ids.sort_by_key(|id| (rank.get(id).copied().unwrap_or(usize::MAX), id.clone()));
        ids
    }

    #[test]
    fn nothing_to_order_leaves_the_seed_order_alone() {
        // The whole no-dependency case: the rank must be the key order, or every
        // tracker that authored no edges would see its listing shuffled.
        assert_eq!(order(&graph(&["c", "a", "b"])), ["a", "b", "c"]);
    }

    #[test]
    fn a_blocker_outranks_what_it_blocks() {
        assert_eq!(order(&graph(&["a ->b", "b"])), ["b", "a"]);
    }

    #[test]
    fn a_constraint_routed_through_another_subtree_still_orders_siblings() {
        // b and c are siblings naming each other nowhere; the ordering runs b -> z -> c,
        // and only a rank computed over the whole graph can see it.
        let g = graph(&["p", "b:p ->z", "c:p", "z ->c"]);
        let rank = sibling_rank(&g, str::to_string);
        assert!(rank.get("c") < rank.get("b"), "{rank:?}");
    }

    #[test]
    fn a_parent_leads_with_the_earliest_row_it_holds() {
        // Epic `z` sorts last on its own key but holds `a`, which sorts first. The epic
        // comes along, ahead of `m`, instead of stranding its own leading row.
        let rank = sibling_rank(&graph(&["z", "a:z", "m"]), str::to_string);
        assert!(rank.get("z") < rank.get("m"), "{rank:?}");
    }

    #[test]
    fn a_parent_keeps_its_own_place_when_nothing_orders_it() {
        // Epic `b` is filed before `c` and only later gains the child `z`. With no edges
        // anywhere the epic holds its own place; ranking it at its subtree's start
        // instead would sink it below `c` in a tracker that authored no dependencies.
        let rank = sibling_rank(&graph(&["b", "z:b", "c"]), str::to_string);
        assert!(rank.get("b") < rank.get("c"), "{rank:?}");
    }

    #[test]
    fn waiting_on_an_epic_waits_for_everything_inside_it() {
        // `a` names only `p`, but a parent is done when its children are, so `a` has to
        // clear `p`'s child too — the target side of the lifting rule.
        assert_eq!(order(&graph(&["a ->p", "p", "z:p"])), ["p", "z", "a"]);
    }

    #[test]
    fn a_finished_blocker_stops_constraining_the_order() {
        // The `needs #NNN` note clears when the blocker is done; the order agrees, and
        // `a` keeps the place `--sort` gave it.
        assert_eq!(order(&graph(&["a ->b", "b @done"])), ["a", "b"]);
    }

    #[test]
    fn a_dependency_a_parent_authored_holds_down_its_whole_subtree() {
        // `p` waits on `z`, so `p`'s child cannot lead `z` either — the lift is what
        // stops the subtree minimum from putting `p` above what it is waiting for.
        let g = graph(&["a:p", "p ->z", "z"]);
        let rank = sibling_rank(&g, str::to_string);
        assert!(rank.get("z") < rank.get("p"), "{rank:?}");
        assert!(rank.get("z") < rank.get("a"), "{rank:?}");
    }

    #[test]
    fn a_dependency_cycle_still_ranks_every_row() {
        // `check` only warns about an effective cycle, so `list` has to render one.
        let g = graph(&["a ->b", "b ->a", "c"]);
        assert_eq!(order(&g).len(), 3);
        assert_eq!(order(&g)[0], "c");
    }

    #[test]
    fn a_parent_cycle_does_not_hang_the_rank() {
        let g = graph(&["a:b", "b:a"]);
        assert_eq!(order(&g).len(), 2);
    }
}
