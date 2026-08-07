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
use crate::render::{Annotation, RowOpts, hl_id, paint, render_rows, unique_prefix_lens};
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

/// `ready --json` / `next --json`: one object, `in_flight` then `ready`.
///
/// Both arrays are flat and unnested, unlike `list --json`: readiness and holding are
/// properties of leaves, so there is no hierarchy to carry. The `ready` order is the
/// ranking — the whole point of the verb.
///
/// The object is the same shape for both verbs, `next` differing only in that `ready`
/// holds at most one row. Where the human view prints the in-flight names only above the
/// one-pick view, the document always carries them: a caller that does not want the
/// context ignores a key, whereas one that does cannot invent it.
pub(crate) fn cmd_ready_json(ctx: &Ctx, root: Option<&str>, only_next: bool) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let root = root.map(|t| resolve_ref(&rows, t)).transpose()?;
    let g = Graph::new(rows);
    let picks = Picks::of(&g, root.as_deref(), only_next);
    let ready: Vec<Json> = picks.ready.iter().filter_map(|id| g.get(id)).map(|r| ranked_row(&g, r)).collect();
    let in_flight: Vec<Json> = picks.in_flight.iter().filter_map(|id| g.get(id)).map(Issue::to_full).collect();
    let doc = vec![("in_flight".to_string(), Json::Array(in_flight)), ("ready".to_string(), Json::Array(ready))];
    Ok(Json::Object(doc).to_json_pretty())
}

/// A ready row as JSON, carrying the demand note as data.
///
/// The note the human view renders as `↑urgent(#a1b2c3)` travels as two fields instead of
/// a coloured string — and only on the rows that carry one: most issues are their own
/// maximum, and emitting nulls everywhere would imply the fields mean something on every
/// row.
fn ranked_row(g: &Graph, r: &Issue) -> Json {
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
}

/// What the verb has to say: the ranked pick list, and the leaves already taken.
///
/// Shared so the two renderings can never disagree about *which* issues are ready, in
/// what order, or what counts as held.
struct Picks {
    ready: Vec<String>,
    in_flight: Vec<String>,
}

impl Picks {
    /// A root id scopes both lists by filtering, never by narrowing the graph they are
    /// computed over — blocking and ranking stay global (see [`cmd_ready`]).
    fn of(g: &Graph, root: Option<&str>, only_next: bool) -> Picks {
        let mut ready = g.ranked_ready();
        let mut in_flight = g.in_flight();
        if let Some(id) = root {
            let kept: BTreeSet<String> = g.subtree(id).into_iter().collect();
            ready.retain(|i| kept.contains(i));
            in_flight.retain(|i| kept.contains(i));
        }
        if only_next {
            ready.truncate(1);
        }
        Picks { ready, in_flight }
    }
}

/// `ready` lists the unblocked leaves in rank order; `next` is the same, capped at one and
/// preceded by the leaves somebody has already started.
///
/// A root id scopes by filtering the *result*, never by restricting the graph readiness
/// and ranking are computed over: blocking is effective, so a leaf here may be waiting on
/// an issue outside the subtree. Narrow the graph and those blockers vanish, making
/// blocked work look actionable.
pub(crate) fn cmd_ready(ctx: &Ctx, root: Option<&str>, only_next: bool) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let root = root.map(|t| resolve_ref(&rows, t)).transpose()?;
    let abbrev = unique_prefix_lens(rows.iter().map(|r| r.id.as_str()));
    let g = Graph::new(rows);
    let picks = Picks::of(&g, root.as_deref(), only_next);

    let rows: Vec<&Issue> = picks.ready.iter().filter_map(|id| g.get(id)).collect();
    let row_opts = RowOpts {
        prefix: None,
        dim: &[],
        on_screen: picks.ready.clone(),
        annotate: Annotation::Demand,
        progress: false,
        show_fields: Vec::new(),
        abbrev: Some(abbrev.clone()),
    };
    // Only above the single pick. The full list already renders every row this line
    // would name, so there it would be a second copy of what is on screen.
    let mut out: Vec<String> = Vec::new();
    if only_next && !picks.in_flight.is_empty() {
        let names: Vec<String> = picks.in_flight.iter().map(|id| hl_id(id, Some(&abbrev), true)).collect();
        out.push(format!("{} {}", paint("in flight:", &["dim"]), names.join(" ")));
    }
    out.extend(render_rows(&g, &rows, &row_opts));
    Ok(out.join("\n"))
}
