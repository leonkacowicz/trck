//! The individual consistency checks.
//!
//! `validate` is a sequence of independent passes, and they live here so that adding one is
//! adding a function to a file about checking rather than making the file that orchestrates
//! them longer. Each takes the graph and appends to the report; none of them writes.

use super::describe_cycle;
use crate::config;
use crate::config::is_terminal;
use crate::graph::Graph;
use std::collections::{BTreeMap, BTreeSet};

/// Rows referring to issues that are not there.
///
/// Split out with the other passes below because `validate` is a sequence of independent
/// checks, and a sequence reads better as its own name per step than as one function with
/// section comments.
pub(super) fn check_references(g: &Graph, by_id: &BTreeSet<&str>, files: &BTreeMap<String, (String, String)>, errors: &mut Vec<String>) {
    for id in files.keys() {
        if !by_id.contains(id.as_str()) {
            errors.push(format!("#{id} markdown file on disk but no index row"));
        }
    }
    for r in &g.rows {
        if let Some(p) = &r.parent
            && !by_id.contains(p.as_str())
        {
            errors.push(format!("#{} parent #{p} does not exist", r.id));
        }
        for dep in &r.depends_on {
            if !by_id.contains(dep.as_str()) {
                errors.push(format!("#{} depends_on #{dep} which does not exist", r.id));
            }
        }
    }
}

/// One error per cycle, not one per node.
///
/// Effective cycles are a superset of the authored ones, and surface inherited deadlocks
/// that arrived by hand-edit, import or `mv`.
pub(super) fn check_cycles(g: &Graph, errors: &mut Vec<String>) {
    for cyc in g.parent_cycles() {
        let mut chain: Vec<String> = cyc.iter().map(|c| format!("#{c}")).collect();
        if let Some(first) = cyc.first() {
            chain.push(format!("#{first}"));
        }
        errors.push(format!("parent cycle: {}", chain.join(" -> ")));
    }
    for cyc in g.effective_cycles() {
        errors.push(format!("effective dependency cycle: {}", describe_cycle(g, &cyc)));
    }
}

/// A non-pinned parent's status must equal the rollup of its children.
///
/// `finalize` maintains this after every verb, so a violation means a hand-edited index.
pub(super) fn check_rollups(g: &Graph, errors: &mut Vec<String>) {
    for r in &g.rows {
        let kids = g.children_of(&r.id);
        if kids.is_empty() || r.manual_status {
            continue;
        }
        let statuses: Vec<String> = kids.iter().filter_map(|k| g.get(k).map(|c| c.status.clone())).collect();
        let desired = config::reconcile(&statuses);
        if r.status != desired {
            errors.push(format!(
                "#{} status '{}' should be '{desired}' (derived from its children; \
                 pin it with a manual `mv` to override)",
                r.id, r.status
            ));
        }
    }
}

/// Finished work that waits on something unfinished. A warning, not an error: it is a
/// question about the tracker rather than a contradiction within it.
pub(super) fn warn_unfinished_dependencies(g: &Graph, warnings: &mut Vec<String>) {
    for r in &g.rows {
        if !is_terminal(&r.status) {
            continue;
        }
        for dep in &r.depends_on {
            if g.get(dep).is_some_and(|d| !is_terminal(&d.status)) {
                warnings.push(format!("#{} is terminal but depends on non-terminal #{dep}", r.id));
            }
        }
    }
}
