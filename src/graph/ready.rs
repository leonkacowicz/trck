//! The predicates: what is blocked, what is free to pick up, and what somebody already holds.
//!
//! All three read the source side of the lifting rule through [`Graph::lifted_deps`]. None of
//! them ranks anything — that is [`super::demand`]'s job.

use super::Graph;
use crate::config;

impl Graph {
    /// One-sided effective blocking: blocked iff this issue, or any ancestor, has an
    /// authored dependency on a non-terminal issue.
    ///
    /// The depended-on side needs no expansion. A parent is terminal only when its whole
    /// subtree is, so "wait for b" already means "wait for everything inside b".
    pub(crate) fn is_blocked(&self, id: &str) -> bool {
        self.lifted_deps(id).iter().any(|b| !self.is_terminal_id(b))
    }

    /// An unblocked leaf nobody has started — work that is genuinely free to pick up.
    ///
    /// `in-progress` and `in-review` both fail this without being terminal. Neither is
    /// available: one is on somebody's desk, the other on somebody's screen. Both still
    /// block whatever waits on them, and both still count toward the demand cone — none of
    /// that is what this predicate answers.
    ///
    /// No `is_terminal` term is needed: [`config::is_actionable`] admits only `backlog`.
    pub(crate) fn is_ready(&self, id: &str) -> bool {
        let Some(r) = self.get(id) else { return false };
        config::is_actionable(&r.status) && self.is_leaf(id) && !self.is_blocked(id)
    }

    /// The leaves somebody is already holding, id-sorted — what `next` names above its
    /// pick so an idle reader can see what is taken without being offered it.
    ///
    /// Leaves only, and deliberately: a parent is `in-progress` because a child is, so
    /// listing it would name a container rather than a claim. Blocking plays no part —
    /// a started issue is held whether or not it is waiting on something.
    pub(crate) fn in_flight(&self) -> Vec<String> {
        let mut out: Vec<String> = self.rows.iter().filter(|r| config::is_in_flight(&r.status) && self.is_leaf(&r.id)).map(|r| r.id.clone()).collect();
        out.sort();
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
    fn readiness_is_leaf_only_unblocked_and_unclaimed() {
        let g = graph(&["epic", "kid:epic", "blocked ->kid", "started @in-progress", "reviewing @in-review", "finished @done", "free"]);
        assert!(g.is_ready("kid"));
        assert!(!g.is_ready("epic"), "a parent is not something you pick up");
        assert!(!g.is_ready("blocked"));
        assert!(!g.is_ready("started"), "somebody already claimed it by starting it");
        assert!(!g.is_ready("reviewing"), "in flight, but its output is pending someone else's judgement");
        assert!(!g.is_ready("finished"));
        assert!(g.is_ready("free"));
    }

    #[test]
    fn in_flight_is_the_started_leaves() {
        let g = graph(&["epic @in-progress", "kid:epic @in-progress", "reviewing @in-review", "waiting ->kid @in-progress", "fresh", "finished @done"]);
        assert_eq!(g.in_flight(), ["kid", "reviewing", "waiting"]);
        // `epic` is in-progress only because `kid` is, so it names no claim of its own.
        assert!(!g.in_flight().contains(&"epic".to_string()));
    }

    #[test]
    fn a_terminal_blocker_stops_blocking() {
        let g = graph(&["dep @done", "work ->dep"]);
        assert!(!g.is_blocked("work"));
        assert!(g.is_ready("work"));
    }

    /// An id that is not in the index is not ready. `is_ready` is asked about ids that came
    /// from elsewhere, and answering `true` on a missing row would offer work nobody has.
    #[test]
    fn a_missing_id_is_not_ready() {
        let g = graph(&["a"]);
        assert!(!g.is_ready("nowhere"));
    }
}
