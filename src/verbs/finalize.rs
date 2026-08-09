//! The single write path every mutating verb ends in.
//!
//! Deriving here rather than in each verb is what makes the rollup uniform across `mv`,
//! `start`, `done`, `new --parent` and re-parenting, with no per-command hooks. Two things are
//! normalised, and both are consequences of a parent being the sum of its children: its
//! `points` are reset, and its status is derived from theirs.

use super::status::apply_status;
use super::write::write_atomic;
use crate::config;
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::index::render_index;
use crate::issue::{DEFAULT_POINTS, Issue};
use crate::summary::generate_summary;
use std::collections::BTreeSet;

/// Persist, regenerate and derive.
pub(crate) fn finalize(ctx: &Ctx, rows: Vec<Issue>) -> Result<Vec<Issue>, String> {
    let mut g = Graph::new(rows);
    reset_parent_points(&mut g);
    derive_parent_statuses(&mut g)?;
    write_atomic(&ctx.index_path(), &render_index(&g.rows))?;
    write_atomic(&ctx.summary_path(), &generate_summary(&g))?;
    report_inconsistencies(ctx, &g.rows);
    Ok(g.rows)
}

/// `points` is a leaf-only input: a parent's weight is the sum of its leaves, so anything
/// stored on a parent would be double-counted and is reset rather than trusted.
///
/// The ids are collected first because the loop needs `&mut g.rows` while `is_leaf` reads `g`.
fn reset_parent_points(g: &mut Graph) {
    let parents: BTreeSet<String> = g.rows.iter().map(|r| r.id.clone()).filter(|id| !g.is_leaf(id)).collect();
    for r in &mut g.rows {
        if parents.contains(&r.id) {
            r.points = DEFAULT_POINTS;
        }
    }
}

/// A parent's status follows its children's, unless it is pinned with `manual_status`.
///
/// Bottom-up over [`postorder`], so a grandparent is settled from children that are already
/// settled; top-down would derive it from statuses about to change underneath it.
fn derive_parent_statuses(g: &mut Graph) -> Result<(), String> {
    for id in postorder(g) {
        let Some(desired) = derived_status(g, &id) else {
            continue;
        };
        set_status(g, &id, desired)?;
    }
    Ok(())
}

/// What `id`'s status should become, or `None` when there is nothing to do — it is a leaf, it
/// is pinned, or it already agrees with its children.
fn derived_status(g: &Graph, id: &str) -> Option<&'static str> {
    let kids = g.children_of(id);
    if kids.is_empty() {
        return None;
    }
    let row = g.get(id)?;
    if row.manual_status {
        return None;
    }
    let statuses: Vec<String> = kids.iter().filter_map(|k| g.get(k).map(|r| r.status.clone())).collect();
    let desired = config::reconcile(&statuses);
    (row.status != desired).then_some(desired)
}

/// Rewrite one row's status and rebuild the graph around it.
///
/// The rebuild is what keeps the walk honest: a parent derived later has to see the status this
/// just wrote, not the one the graph was built with.
fn set_status(g: &mut Graph, id: &str, status: &str) -> Result<(), String> {
    let mut rows = std::mem::take(&mut g.rows);
    if let Some(row) = rows.iter_mut().find(|r| r.id == id) {
        apply_status(row, status)?;
    }
    *g = Graph::new(rows);
    Ok(())
}

/// Rows ordered children-before-parents, so a bottom-up pass sees each node's
/// descendants already settled.
fn postorder(g: &Graph) -> Vec<String> {
    let mut out = Vec::new();
    let mut seen = BTreeSet::new();
    for r in &g.rows {
        push_subtree(g, &r.id, &mut seen, &mut out);
    }
    out
}

/// Explicit stack with a visit flag; recursion would blow up on the deep hierarchy a
/// malformed index can produce, and `seen` is what stops a parent cycle from spinning.
fn push_subtree(g: &Graph, root: &str, seen: &mut BTreeSet<String>, out: &mut Vec<String>) {
    let mut stack = vec![(root.to_string(), false)];
    while let Some((id, expanded)) = stack.pop() {
        if expanded {
            out.push(id);
        } else if seen.insert(id.clone()) {
            stack.push((id.clone(), true));
            for kid in g.children_of(&id) {
                stack.push((kid.clone(), false));
            }
        }
    }
}

/// Validate what was just written, reusing the rows rather than re-parsing.
///
/// A verb that leaves the tracker inconsistent still succeeds — it did what it was asked — but
/// says so loudly, because the next thing that runs is usually a commit.
fn report_inconsistencies(ctx: &Ctx, rows: &[Issue]) {
    let Ok(report) = crate::validate::validate(ctx, rows) else {
        return;
    };
    for w in &report.warnings {
        eprintln!("warning: {w}");
    }
    if report.errors.is_empty() {
        return;
    }
    eprintln!("\nINCONSISTENCIES after this operation:");
    for e in &report.errors {
        eprintln!("  error: {e}");
    }
    eprintln!("the tracker is now inconsistent — fix before committing.");
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;
    use crate::test_graph::graph;

    /// Children before parents, and a grandchild before its parent before the root.
    #[test]
    fn postorder_puts_every_child_before_its_parent() {
        let g = graph(&["root", "mid:root", "leaf:mid", "other:root"]);
        let order = postorder(&g);
        let at = |id: &str| order.iter().position(|x| x == id).unwrap_or(usize::MAX);
        assert!(at("leaf") < at("mid"), "{order:?}");
        assert!(at("mid") < at("root"), "{order:?}");
        assert!(at("other") < at("root"), "{order:?}");
        assert_eq!(order.len(), 4, "each node once: {order:?}");
    }

    /// A parent cycle is malformed data `check` reports — the walk must terminate before the
    /// user ever gets there, and must still list each node once.
    #[test]
    fn postorder_does_not_spin_on_a_parent_cycle() {
        let g = graph(&["a:b", "b:a"]);
        let order = postorder(&g);
        assert_eq!(order.len(), 2, "{order:?}");
    }

    /// A parent's stored points are ignored rather than trusted; a leaf's are its own.
    #[test]
    fn a_parents_points_are_reset_and_a_leafs_are_left() {
        let mut g = graph(&["root #99", "leaf:root #4"]);
        reset_parent_points(&mut g);
        let points = |id: &str| g.get(id).map(|r| r.points);
        assert_eq!(points("root"), Some(DEFAULT_POINTS), "a parent carries no points of its own");
        assert_eq!(points("leaf"), Some(4), "a leaf keeps its weight");
    }

    /// The derivation: all children initial means initial, all terminal means terminal, a mix
    /// means active.
    #[test]
    fn a_parents_status_follows_its_children() {
        let all_open = graph(&["root", "a:root", "b:root"]);
        assert_eq!(derived_status(&all_open, "root"), None, "already agrees");

        let mixed = graph(&["root", "a:root @done", "b:root"]);
        assert_eq!(derived_status(&mixed, "root"), Some(config::IN_PROGRESS));

        let all_done = graph(&["root", "a:root @done", "b:root @done"]);
        assert_eq!(derived_status(&all_done, "root"), Some(config::DONE));
    }

    #[test]
    fn a_leaf_derives_nothing() {
        let g = graph(&["lonely"]);
        assert_eq!(derived_status(&g, "lonely"), None);
    }

    /// `manual_status` is the escape hatch: a pinned parent keeps the status someone chose.
    #[test]
    fn a_pinned_parent_is_left_alone() {
        let mut g = graph(&["root", "a:root @done"]);
        let mut rows = std::mem::take(&mut g.rows);
        if let Some(row) = rows.iter_mut().find(|r| r.id == "root") {
            row.manual_status = true;
        }
        let g = Graph::new(rows);
        assert_eq!(derived_status(&g, "root"), None, "a pinned parent must not be re-derived");
    }

    /// A grandparent must settle from children this same pass already settled — the reason the
    /// walk is bottom-up and rebuilds the graph as it goes.
    #[test]
    fn derivation_reaches_a_grandparent_in_one_pass() {
        let mut g = graph(&["root", "mid:root", "leaf:mid @done"]);
        derive_parent_statuses(&mut g).expect("derives");
        assert_eq!(g.get("mid").map(|r| r.status.as_str()), Some(config::DONE), "mid follows its only child");
        assert_eq!(g.get("root").map(|r| r.status.as_str()), Some(config::DONE), "and root follows mid in the same pass");
    }
}
