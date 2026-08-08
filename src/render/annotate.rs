//! The trailing note a row carries: what it is waiting on, or why it ranks where it does.
//!
//! Both occupy the same slot at the end of a row, and no view wants both — `list` explains
//! waiting, `ready` explains ranking. Which one a view asks for is [`Annotation`].

use super::{hl_id, paint, priority_codes};
use crate::config::is_terminal;
use crate::graph::Graph;
use std::collections::BTreeMap;

/// The trailing note a view attaches to each row. `list` explains what a row is waiting
/// on; `ready` explains why it ranks where it does. They occupy the same slot, and no
/// view wants both.
#[derive(PartialEq, Eq)]
pub(crate) enum Annotation {
    None,
    Blocking,
    Demand,
}

/// The dim `needs #… blocks #…` suffix explaining why a row is waiting, and what is
/// waiting on it.
pub(crate) fn block_annotations(g: &Graph, id: &str, on_screen: &[String]) -> String {
    let mut parts: Vec<String> = Vec::new();
    let needs = needs_note(g, id, on_screen);
    if !needs.is_empty() {
        parts.push(format!("needs {}", needs.join(" ")));
    }
    if let Some(blocks) = blocks_note(g, id) {
        parts.push(blocks);
    }
    if parts.is_empty() { String::new() } else { paint(&format!(" {}", parts.join("  ")), &["dim"]) }
}

/// The non-terminal blockers, nearest author first.
///
/// An inherited one is tagged `(via #author)` so the note never implies the edge was authored
/// here — that is where `dep --remove` goes. It is spelled out only when no row between this
/// one and the author is itself being printed: where such a row exists it already carries the
/// note, and restating it down every child is noise.
fn needs_note(g: &Graph, id: &str, on_screen: &[String]) -> Vec<String> {
    let spine = g.ancestors_of(id);
    let mut needs: Vec<String> = Vec::new();
    for author in std::iter::once(id.to_string()).chain(g.ancestors_of(id)) {
        let Some(via) = author_suffix(&spine, on_screen, &author, id) else {
            continue;
        };
        for target in g.requires_of(&author) {
            // A target reached twice keeps its nearest author; a done blocker drops off
            // entirely, because the block is cleared.
            if !already_noted(&needs, &target) && !is_done(g, &target) {
                needs.push(format!("#{target}{via}"));
            }
        }
    }
    needs
}

/// How one author's edges should be labelled, or `None` when they should stay quiet.
///
/// The answer does not depend on the target, so it is decided once per author rather than once
/// per edge that author wrote.
fn author_suffix(spine: &[String], on_screen: &[String], author: &str, id: &str) -> Option<String> {
    if author == id {
        return Some(String::new()); // authored here; named plainly
    }
    if carried_above(spine, on_screen, author) {
        return None;
    }
    Some(format!(" (via #{author})"))
}

/// Is a printed row between this one and `author` (inclusive) already saying it?
fn carried_above(spine: &[String], on_screen: &[String], author: &str) -> bool {
    for a in spine {
        if on_screen.contains(a) {
            return true;
        }
        if a == author {
            break;
        }
    }
    false
}

/// Whether this target already has a note, matched on the id rather than the whole string so
/// a `(via #…)` form counts as the same target.
fn already_noted(needs: &[String], target: &str) -> bool {
    needs.iter().any(|n| n.contains(&format!("#{target}")))
}

fn is_done(g: &Graph, id: &str) -> bool {
    g.get(id).is_some_and(|r| is_terminal(&r.status))
}

/// What waits on this row, at the **authored** altitude rather than mirroring the lifting:
/// those dependents' subtrees inherit the wait, and are exactly the rows whose `needs` reads
/// `(via #…)`. A finished row blocks nothing.
fn blocks_note(g: &Graph, id: &str) -> Option<String> {
    if is_done(g, id) {
        return None;
    }
    let blocks = g.dependents_of(id);
    (!blocks.is_empty()).then(|| format!("blocks {}", blocks.iter().map(|d| format!("#{d}")).collect::<Vec<_>>().join(" ")))
}

/// The ` ↑<priority>(#id)` suffix naming why a row outranks its own priority: the
/// highest-priority issue waiting on it, coloured as that priority. Empty when the row is
/// its own maximum, which most rows are.
///
/// `ready` sorts by the demand cone, so without this a `medium` row sits above a `high`
/// one with nothing on screen to explain it. It rides the same trailing slot `list` uses
/// for its `needs`/`blocks` notes rather than widening the priority column.
pub(crate) fn demand_annotation(g: &Graph, id: &str, abbrev: Option<&BTreeMap<String, usize>>) -> String {
    let Some(src) = g.demand_source(id) else {
        return String::new();
    };
    let Some(row) = g.get(&src) else {
        return String::new();
    };
    format!("  {}({})", paint(&format!("↑{}", row.priority), &priority_codes(&row.priority)), hl_id(&src, abbrev, true))
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::test_graph::graph;

    fn note(spec: &[&str], id: &str, on_screen: &[&str]) -> String {
        let g = graph(spec);
        let screen: Vec<String> = on_screen.iter().map(|s| (*s).to_string()).collect();
        block_annotations(&g, id, &screen)
    }

    #[test]
    fn an_authored_blocker_is_named_plainly() {
        assert!(note(&["dep", "work ->dep"], "work", &[]).contains("needs #dep"));
    }

    /// A done blocker drops off entirely: the note explains why a row is *waiting*, and it
    /// is not.
    #[test]
    fn a_finished_blocker_leaves_no_note() {
        assert_eq!(note(&["dep @done", "work ->dep"], "work", &[]), "");
    }

    /// An inherited edge says whose it is, so nobody looks for it on the wrong issue.
    #[test]
    fn an_inherited_blocker_names_its_author() {
        let out = note(&["dep", "epic ->dep", "kid:epic"], "kid", &[]);
        assert!(out.contains("needs #dep (via #epic)"), "{out}");
    }

    /// ...unless the author, or a row between, is on screen — that row already carries it,
    /// and repeating it down every child is noise.
    #[test]
    fn an_inherited_blocker_stays_quiet_when_a_printed_row_carries_it() {
        let out = note(&["dep", "epic ->dep", "kid:epic"], "kid", &["epic"]);
        assert!(!out.contains("via"), "{out}");
    }

    #[test]
    fn a_target_reached_twice_keeps_its_nearest_author() {
        // `kid` authors the edge itself and inherits the same one from `epic`.
        let out = note(&["dep", "epic ->dep", "kid:epic ->dep"], "kid", &[]);
        assert!(out.contains("needs #dep"), "{out}");
        assert!(!out.contains("via"), "the nearer author wins: {out}");
    }

    #[test]
    fn blocks_lists_the_dependents_and_a_done_row_blocks_nothing() {
        let out = note(&["dep", "a ->dep", "b ->dep"], "dep", &[]);
        assert!(out.contains("blocks #a #b"), "{out}");
        assert_eq!(note(&["dep @done", "a ->dep"], "dep", &[]), "", "a finished row blocks nothing");
    }

    #[test]
    fn both_halves_appear_together() {
        let out = note(&["low", "mid ->low", "top ->mid"], "mid", &[]);
        assert!(out.contains("needs #low"), "{out}");
        assert!(out.contains("blocks #top"), "{out}");
    }

    /// Most rows carry nothing, and the empty case must be genuinely empty — not a stray
    /// space, which would show up as trailing whitespace in every golden.
    #[test]
    fn an_unrelated_row_carries_no_note_at_all() {
        assert_eq!(note(&["lonely"], "lonely", &[]), "");
    }

    #[test]
    fn demand_names_the_issue_that_lifts_a_row_and_is_empty_at_the_maximum() {
        let g = graph(&["blocker !medium", "urgent ->blocker !urgent"]);
        let out = demand_annotation(&g, "blocker", None);
        assert!(out.contains("↑urgent"), "{out}");
        assert!(out.contains("urgent"), "{out}");
        assert_eq!(demand_annotation(&g, "urgent", None), "", "already its own maximum");
    }
}
