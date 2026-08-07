//! The read verbs: `list` (and its `tree` alias), `show`, and the path verbs beside them.
//!
//! `list`'s default is a nested forest that **hides settled work**: a done issue shows
//! only while it is still open or sits directly under a non-terminal parent, so an open
//! epic keeps its done children as progress context but a finished subtree drops off.
//! `--all` or an explicit `--status` bypasses that.
//!
//! The other thing worth knowing is the *closure*: a filtered forest shows a node when
//! it matches **or has a matching descendant**, and the non-matching ancestors come along
//! as dimmed context. Without that a matched child floats free of the epic it belongs to.

mod deps;
mod list;
mod paths;
mod rank;
mod show;
pub(crate) use deps::{DepsOpts, cmd_deps, cmd_deps_json};
pub(crate) use list::cmd_list;
pub(crate) use paths::{cmd_path, cmd_which, which_operands};
pub(crate) use show::{cmd_show, cmd_show_json};

use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::issue::Issue;
use crate::json::Json;
use crate::render::{Annotation, RowOpts, render_rows, unique_prefix_lens};
use crate::verbs::{load_rows, resolve_ref};
use std::collections::BTreeSet;

/// Everything `list` accepts.
///
/// Five booleans, which clippy dislikes and which is right anyway: they mirror the CLI
/// flags one-to-one, and folding them into an enum would hide that `--flat --paths` is a
/// combination the caller can express and this code has to answer for.
#[allow(clippy::struct_excessive_bools, reason = "mirrors the CLI flags one-to-one")]
#[derive(Default)]
pub(crate) struct ListOpts<'a> {
    pub(crate) root: Option<&'a str>,
    pub(crate) status: Option<&'a str>,
    pub(crate) priority: Option<&'a str>,
    pub(crate) label: Option<&'a str>,
    pub(crate) parent: Option<&'a str>,
    pub(crate) match_title: Option<&'a str>,
    pub(crate) fields: Vec<&'a str>,
    pub(crate) show_fields: Vec<&'a str>,
    pub(crate) sort: Option<&'a str>,
    pub(crate) blocked: bool,
    pub(crate) orphan: bool,
    pub(crate) all: bool,
    pub(crate) flat: bool,
    pub(crate) paths: bool,
    pub(crate) json: bool,
}

/// `ready` lists the unblocked leaves in rank order; `next` is the same, capped at one.
///
/// A root id scopes by filtering the *result*, never by restricting the graph readiness
/// and ranking are computed over: blocking is effective, so a leaf here may be waiting on
/// an issue outside the subtree. Narrow the graph and those blockers vanish, making
/// blocked work look actionable.
/// `ready --json` / `next --json`: the ranked actionable rows, as a flat array.
///
/// Flat and unnested, unlike `list --json`: readiness is a property of leaves, so there is
/// no hierarchy to carry. The order is the ranking — the whole point of the verb — and no
/// `↑demand` marker appears, because the caller can compute it from the rows.
pub(crate) fn cmd_ready_json(ctx: &Ctx, root: Option<&str>, only_next: bool) -> Result<String, String> {
    let ids = ready_ids(ctx, root, only_next)?;
    let rows = load_rows(ctx)?;
    let g = Graph::new(rows);
    // The array order is this verb's whole answer, so a consumer never re-derives the
    // ranking. The note the human view renders as `↑urgent(#a1b2c3)` travels as two fields
    // instead of a coloured string — and only on the rows that carry one: most issues are
    // their own maximum, and emitting nulls everywhere would imply the fields mean
    // something on every row.
    let out: Vec<Json> = ids
        .iter()
        .filter_map(|id| g.get(id))
        .map(|r| {
            let mut obj = match r.to_full() {
                Json::Object(pairs) => pairs,
                other => return other,
            };
            if let Some(src) = g.demand_source(&r.id)
                && let Some(row) = g.get(&src)
            {
                obj.push(("demand_priority".into(), Json::String(row.priority.clone())));
                obj.push(("demand_source".into(), Json::String(src.clone())));
            }
            Json::Object(obj)
        })
        .collect();
    Ok(Json::Array(out).to_json_pretty())
}

/// The ranked ready set, scoped to a subtree and truncated for `next` — shared so the two
/// renderings can never disagree about *which* issues are ready or in what order.
fn ready_ids(ctx: &Ctx, root: Option<&str>, only_next: bool) -> Result<Vec<String>, String> {
    let rows = load_rows(ctx)?;
    let root = root.map(|t| resolve_ref(&rows, t)).transpose()?;
    let g = Graph::new(rows);
    let mut ids = g.ranked_ready();
    if let Some(id) = &root {
        let kept: BTreeSet<String> = g.subtree(id).into_iter().collect();
        ids.retain(|i| kept.contains(i));
    }
    if only_next {
        ids.truncate(1);
    }
    Ok(ids)
}

pub(crate) fn cmd_ready(ctx: &Ctx, root: Option<&str>, only_next: bool) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let root = root.map(|t| resolve_ref(&rows, t)).transpose()?;
    let abbrev = unique_prefix_lens(rows.iter().map(|r| r.id.as_str()));
    let g = Graph::new(rows);

    let mut ids = g.ranked_ready();
    if let Some(id) = &root {
        let kept: BTreeSet<String> = g.subtree(id).into_iter().collect();
        ids.retain(|i| kept.contains(i));
    }
    if only_next {
        ids.truncate(1);
    }
    let rows: Vec<&Issue> = ids.iter().filter_map(|id| g.get(id)).collect();
    let row_opts = RowOpts {
        prefix: None,
        dim: &[],
        on_screen: ids.clone(),
        annotate: Annotation::Demand,
        progress: false,
        show_fields: Vec::new(),
        abbrev: Some(abbrev),
    };
    Ok(render_rows(&g, &rows, &row_opts).join("\n"))
}
