//! `mv` (and its `start`/`review`/`done` aliases): the verb that moves an issue through the
//! workflow.
//!
//! The subtlety is the rollup. A parent's status is normally derived from its children, so a
//! move that disagrees with them has to *pin* the row — and a move that agrees with them has
//! to leave it unpinned, or the next child to move would be ignored.

use super::super::{Op, apply_status, body_location, commit, load_rows, resolve_ref};
use crate::config::{self, is_terminal};
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::issue::Issue;

/// What a move records besides the destination: the two fields the aliases fill in.
#[derive(Default)]
pub(crate) struct MvOpts<'a> {
    pub(crate) status: &'a str,
    pub(crate) resolution: Option<&'a str>,
    pub(crate) review_url: Option<&'a str>,
}

pub(crate) fn cmd_mv(ctx: &Ctx, token: &str, opts: &MvOpts) -> Result<String, String> {
    let mut rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    check_mv_opts(opts)?;
    // Asked of the tracker, not of a filesystem: the wording is contract, and it is the
    // accessor that knows whether a body is a file or a tree entry. The content is discarded —
    // what `mv` wants from it is the refusal when there is nothing there.
    let where_ = body_where(ctx, &mut rows, &iid)?;
    let kid_statuses = child_statuses(&mut rows, &iid);
    if let Some(row) = rows.iter_mut().find(|r| r.id == iid) {
        apply_move(row, opts, &kid_statuses)?;
    }
    // Canonical `mv`, never the alias: `start`/`review`/`done` are spellings of a status, and
    // recording the spelling would make replay depend on the vocabulary of whoever typed it.
    let op = Op::new("mv").operand(&iid).operand(opts.status).flag("--resolution", opts.resolution).flag("--review-url", opts.review_url);
    commit(ctx, rows, Vec::new(), &op)?;
    Ok(where_)
}

/// Refuse an option combination before anything moves. A resolution says *how* an issue
/// ended, so it only means something on a terminal status; the rest is vocabulary.
fn check_mv_opts(opts: &MvOpts) -> Result<(), String> {
    if let Some(res) = opts.resolution {
        if !is_terminal(opts.status) {
            return Err("--resolution is only valid when moving to a terminal status".into());
        }
        if let Some(msg) = config::check_resolution(res) {
            return Err(msg);
        }
    }
    if let Some(url) = opts.review_url
        && let Some(msg) = config::check_review_url(url)
    {
        return Err(msg);
    }
    config::check_status(opts.status).map_or(Ok(()), Err)
}

/// Where the issue's body is, having confirmed that it is there at all.
///
/// Takes the rows by `&mut` and hands them back because the answer comes from the graph's
/// view of the row, and the graph owns them while it exists.
fn body_where(ctx: &Ctx, rows: &mut Vec<Issue>, iid: &str) -> Result<String, String> {
    let g = Graph::new(std::mem::take(rows));
    let found = g.get(iid).map(|r| ctx.read_body(r).map(|_| body_location(ctx, r)));
    *rows = g.rows;
    found.ok_or_else(|| format!("no issue matching '{iid}'"))?
}

/// The statuses of the issue's children — what derivation would have produced, and so what
/// the move is measured against.
fn child_statuses(rows: &mut Vec<Issue>, iid: &str) -> Vec<String> {
    let g = Graph::new(std::mem::take(rows));
    let statuses = g.children_of(iid).iter().filter_map(|k| g.get(k).map(|r| r.status.clone())).collect();
    *rows = g.rows;
    statuses
}

/// The move itself, once everything about it is known to be legal.
fn apply_move(row: &mut Issue, opts: &MvOpts, kid_statuses: &[String]) -> Result<(), String> {
    apply_status(row, opts.status)?;
    if let Some(url) = opts.review_url {
        row.review_url = Some(url.to_string());
    }
    if let Some(res) = opts.resolution {
        row.resolution = Some(res.to_string());
    }
    // Moving a node with children overrides the rollup — but only when the requested
    // status differs from what derivation would produce. A move that agrees with the
    // children leaves it unpinned, so nothing to override.
    if !kid_statuses.is_empty() {
        row.manual_status = row.status != config::reconcile(kid_statuses);
    }
    Ok(())
}
