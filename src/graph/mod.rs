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
//! Every derived answer here is that rule read in one direction or the other, and the
//! modules are named for the direction they read it in: [`deps`] reads the source side,
//! [`ready`] turns it into a predicate, [`demand`] reads it reversed, and the cycle checks
//! in [`cycles`] compose both. [`hierarchy`] is the containment those all climb, and
//! [`rollup`] is the one answer that only descends it.
//!
//! Every module here adds methods to the one [`Graph`] — the struct and its construction
//! live here, and nothing else does.

mod cycles;
mod demand;
mod deps;
mod hierarchy;
mod ready;
mod rollup;

use crate::config::{self, is_terminal};
use crate::issue::Issue;
use std::collections::BTreeMap;

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
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use std::fmt::Write as _;

    /// Answer every derived question over this repo's real graph and dump it, so another
    /// engine's answers — or this one's, before a refactor — can be diffed against it.
    ///
    /// The unit tests beside each module cover shapes someone thought of. This covers a
    /// graph with real epics, real inherited edges and a real ranking — the place where a
    /// subtly wrong lifting rule shows up and a hand-written fixture would not. It lives
    /// here rather than in one of the modules because it reads all of them at once.
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
    fn an_unrecognised_priority_sinks_rather_than_blowing_up() {
        assert_eq!(priority_rank("nonesuch"), config::PRIORITIES.len());
        let g = crate::test_graph::graph(&["junk !nonesuch", "ok !lowest"]);
        assert_eq!(g.ranked_ready(), ["ok", "junk"]);
    }
}
