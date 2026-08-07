//! `label` and `dep`: the two verbs that edit a list on an issue rather than a scalar.
//!
//! Both are add-and-remove in one call, both keep the list sorted so the index does not
//! churn on ordering, and both report the resulting list back. The difference is that a
//! label means nothing to anything else, while a dependency edge can close a cycle — so
//! `dep` guards against the candidate graph and `label` has nothing to guard.

use super::super::{finalize, load_rows, resolve_ref};
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::issue::Issue;
use crate::render::python_list;

pub(crate) fn cmd_label(ctx: &Ctx, token: &str, add: &[&str], remove: &[&str]) -> Result<String, String> {
    let mut rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    let Some(row) = rows.iter_mut().find(|r| r.id == iid) else {
        return Err(format!("no issue matching '{iid}'"));
    };
    for lab in add {
        if !lab.is_empty() && !row.labels.iter().any(|l| l == lab) {
            row.labels.push((*lab).to_string());
        }
    }
    row.labels.retain(|l| !remove.contains(&l.as_str()));
    row.labels.sort();
    let shown = python_list(&row.labels);
    finalize(ctx, rows)?;
    Ok(format!("#{iid} labels={shown}"))
}

pub(crate) fn cmd_dep(ctx: &Ctx, token: &str, add: Option<&str>, remove: Option<&str>) -> Result<String, String> {
    let mut rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    let add = add.map(|a| resolve_ref(&rows, a)).transpose()?;
    let remove = remove.map(|r| resolve_ref(&rows, r)).transpose()?;
    guard_edge(&mut rows, &iid, add.as_deref())?;
    let Some(row) = rows.iter_mut().find(|r| r.id == iid) else {
        return Err(format!("no issue matching '{iid}'"));
    };
    apply_edge(row, add, remove);
    let shown = python_list(&row.depends_on);
    finalize(ctx, rows)?;
    Ok(format!("#{iid} depends_on={shown}"))
}

/// Refuse an edge that would close a cycle — directly or through the hierarchy — against the
/// graph as it stands, before the row is touched. Removing an edge can never close one, so
/// only `--add` is guarded.
fn guard_edge(rows: &mut Vec<Issue>, iid: &str, add: Option<&str>) -> Result<(), String> {
    let Some(target) = add else { return Ok(()) };
    let g = Graph::new(std::mem::take(rows));
    let refusal = g.check_dep_edge(iid, target);
    *rows = g.rows;
    refusal.map_or(Ok(()), Err)
}

/// Add and remove in one pass, then sort — the list is a set, so a repeat add is a no-op and
/// the stored order never depends on the order edges were authored in.
fn apply_edge(row: &mut Issue, add: Option<String>, remove: Option<String>) {
    if let Some(target) = add
        && !row.depends_on.contains(&target)
    {
        row.depends_on.push(target);
    }
    if let Some(target) = remove {
        row.depends_on.retain(|d| *d != target);
    }
    row.depends_on.sort();
}
