//! The read verbs: `list` (and its `tree` alias) and `show`.
//!
//! `list`'s default is a nested forest that **hides settled work**: a done issue shows
//! only while it is still open or sits directly under a non-terminal parent, so an open
//! epic keeps its done children as progress context but a finished subtree drops off.
//! `--all` or an explicit `--status` bypasses that.
//!
//! The other thing worth knowing is the *closure*: a filtered forest shows a node when
//! it matches **or has a matching descendant**, and the non-matching ancestors come along
//! as dimmed context. Without that a matched child floats free of the epic it belongs to.

mod list;
pub(crate) use list::cmd_list;

use crate::config::is_terminal;
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::gutter;
use crate::issue::{CANON_KEYS, Issue};
use crate::json::Json;
use crate::render::{
    Annotation, LANE_PALETTE, RowOpts, field_value_raw, hl_id, lane_palette_index, paint, render_rows, status_codes, status_icon, unique_prefix_lens,
};
use crate::verbs::{issue_path, load_rows, resolve_ref};
use std::collections::{BTreeMap, BTreeSet};

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

/// Options `deps` accepts.
#[allow(clippy::struct_excessive_bools, reason = "mirrors the CLI flags one-to-one")]
#[derive(Default)]
pub(crate) struct DepsOpts<'a> {
    pub(crate) root: Option<&'a str>,
    pub(crate) requires: bool,
    pub(crate) blocks: bool,
    pub(crate) full: bool,
    pub(crate) omit_done: bool,
    pub(crate) include_done_chains: bool,
    pub(crate) fanout: bool,
}

/// `deps --json`: one issue's two cones, as `{requires, blocks}`.
///
/// Needs an id. The whole-graph view is an edge list — a different shape entirely — and
/// silently returning one under the same key names would be worse than refusing.
///
/// Rows are emitted in index order rather than the order the cone walk happens to produce:
/// the walk works off a set, so its iteration order is not something a golden file could
/// survive.
pub(crate) fn cmd_deps_json(ctx: &Ctx, opts: &DepsOpts) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let Some(token) = opts.root else {
        return Err("deps --json needs an issue id (the whole-graph view is an edge \
                    list, a different shape from one issue's cones)"
            .into());
    };
    let iid = resolve_ref(&rows, token)?;
    let g = Graph::new(rows);
    let cone = |up: bool, down: bool| -> Vec<Json> {
        let line = g.dependency_line(&iid, up, down);
        g.rows.iter().filter(|r| r.id != iid && line.contains(&r.id)).map(Issue::to_full).collect()
    };
    Ok(Json::Object(vec![("requires".into(), Json::Array(cone(true, false))), ("blocks".into(), Json::Array(cone(false, true)))]).to_json_pretty())
}

pub(crate) fn cmd_deps(ctx: &Ctx, opts: &DepsOpts) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let root = opts.root.map(|t| resolve_ref(&rows, t)).transpose()?;
    if (opts.requires || opts.blocks) && root.is_none() {
        return Err("deps: --requires/--blocks scope one issue's graph — pass an issue id".into());
    }
    let abbrev = unique_prefix_lens(rows.iter().map(|r| r.id.as_str()));
    let g = Graph::new(rows);

    // Default (neither flag) shows both cones; one flag scopes to that direction.
    let up = opts.requires || !opts.blocks;
    let down = opts.blocks || !opts.requires;

    let ids: BTreeSet<String> = if let Some(id) = &root {
        let has_edges = !g.requires_of(id).is_empty()
            || !g.dependents_of(id).is_empty()
            || !g.children_of(id).is_empty()
            || g.get(id).and_then(|r| r.parent.clone()).is_some();
        if !has_edges {
            if opts.omit_done && g.get(id).is_some_and(|r| is_terminal(&r.status)) {
                return Ok(String::new());
            }
            let Some(row) = g.get(id) else {
                return Err(format!("no issue matching '{id}'"));
            };
            return Ok(format!("{}  (no dependencies)", node_label(&g, row, true, Some(&abbrev))));
        }
        if opts.full {
            // The focal node's whole component, computed over *every* issue — not over
            // the overview set, which drops the components the bare view suppresses and
            // could therefore lose the focal node itself.
            let all: BTreeSet<String> = g.rows.iter().map(|r| r.id.clone()).collect();
            let edges = gutter::drawn_edges(&g, &all, false, false);
            gutter::components(&all, &edges).into_iter().find(|c| c.contains(id)).unwrap_or_default().into_iter().collect()
        } else {
            g.dependency_line(id, up, down)
        }
    } else {
        let ids = gutter::overview_ids(&g);
        if ids.is_empty() {
            return Ok("no dependencies recorded yet".into());
        }
        ids
    };
    let ids = gutter::filter_done(&g, &ids, opts.omit_done, opts.include_done_chains, root.is_none());

    let rendered = gutter::render_graph(&g, &ids, opts.fanout);
    let width = rendered.iter().flatten().map(|(_, gut, _)| gut.chars().count()).max().unwrap_or(0);
    let mut out: Vec<String> = Vec::new();
    for row in &rendered {
        let Some((iid, gut, owners)) = row else {
            out.push(String::new());
            continue;
        };
        let focal = root.as_deref() == Some(iid.as_str());
        // A left-margin caret marks the focal row; a blank 2-column margin on every
        // other row keeps the graph aligned. The whole-graph view has no focal node.
        let marker = match &root {
            None => String::new(),
            Some(_) if focal => format!("{} ", paint("▸", &["bold"])),
            Some(_) => "  ".to_string(),
        };
        let painted = paint_lanes(gut, owners);
        let Some(row) = g.get(iid) else { continue };
        out.push(format!("{marker}{painted}{}  {}", " ".repeat(width - gut.chars().count()), node_label(&g, row, focal, Some(&abbrev))));
    }
    Ok(out.join("\n"))
}

/// A gutter row with each lane coloured by a rotating palette keyed on the id it heads to,
/// so a lane keeps one hue for its whole descent and can be traced through crossings. An
/// inferred containment edge is dimmed *on top of* its hue — weight, not colour, marks it as
/// structure — since box-drawing has no dashed corner to distinguish it by shape. The node's
/// own bullet (`●`) is bold rather than palette-coloured.
fn paint_lanes(gut: &str, owners: &[gutter::LaneOwner]) -> String {
    gut.chars()
        .zip(owners.iter())
        .map(|(ch, owner)| {
            if ch == '●' {
                return paint("●", &["bold"]);
            }
            match owner {
                None => ch.to_string(),
                Some((id, kind)) => {
                    let mut codes = vec![LANE_PALETTE[lane_palette_index(id)]];
                    if *kind == gutter::EdgeKind::Child {
                        codes.insert(0, "dim");
                    }
                    paint(&ch.to_string(), &codes)
                },
            }
        })
        .collect()
}

/// One node's label in the graph: id, status icon, title, and a derived epic marker.
///
/// `·epic·` comes from the hierarchy, not from a stored kind — an issue with children
/// *is* an epic, and a declared marker only drifts from that.
fn node_label(g: &Graph, r: &Issue, focal: bool, abbrev: Option<&BTreeMap<String, usize>>) -> String {
    let tag = if g.children_of(&r.id).is_empty() { String::new() } else { " ·epic·".to_string() };
    let labels = if r.labels.is_empty() { String::new() } else { paint(&format!(" [{}]", r.labels.join(" ")), &["dim"]) };
    let emph: &[&str] = if focal { &["bold"] } else { &[] };
    format!("{} {} {}{tag}{labels}", hl_id(&r.id, abbrev, true), paint(status_icon(&r.status), &status_codes(&r.status)), paint(&r.title, emph))
}

/// `show --json`: one document with the body folded in.
///
/// Metadata *and* body together, rather than the human view's metadata-then-separator: the
/// obvious way to consume this is `json.loads(stdout)`, and a trailing `--- body ---` block
/// would break it. `points` is dropped on a parent for the same reason the human view drops
/// it — there it is derived, not an input.
pub(crate) fn cmd_show_json(ctx: &Ctx, token: &str) -> Result<String, String> {
    let (row, body, is_leaf) = show_parts(ctx, token)?;
    let mut obj = match row.to_full() {
        Json::Object(pairs) => pairs,
        _ => Vec::new(),
    };
    if !is_leaf {
        obj.retain(|(k, _)| k != "points");
    }
    obj.push(("body".into(), Json::String(body)));
    Ok(Json::Object(obj).to_json_pretty())
}

/// The row, its body text and whether it is a leaf — everything both `show` renderings need,
/// resolved and guarded once so the two cannot disagree about which issue they are showing.
fn show_parts(ctx: &Ctx, token: &str) -> Result<(Issue, String, bool), String> {
    let rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    let g = Graph::new(rows);
    let row = g.get(&iid).ok_or_else(|| format!("no issue matching '{iid}'"))?.clone();
    let path = issue_path(ctx, &row);
    if !path.exists() {
        return Err(format!("file missing for #{}: {}", row.id, path.display()));
    }
    let body = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok((row, body, g.is_leaf(&iid)))
}

pub(crate) fn cmd_show(ctx: &Ctx, token: &str) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    let abbrev = unique_prefix_lens(rows.iter().map(|r| r.id.as_str()));
    let g = Graph::new(rows);
    let row = g.get(&iid).ok_or_else(|| format!("no issue matching '{iid}'"))?;

    let mut keys: Vec<String> = CANON_KEYS.iter().map(|k| (*k).to_string()).collect();
    if !g.is_leaf(&iid) {
        // Points roll up from leaves, so on a parent the stored value is not an input.
        keys.retain(|k| k != "points");
    }
    keys.extend(row.extra.keys().cloned());

    // The column width comes from *every* candidate key, not only the ones with a
    // value. An issue that happens to carry no `manual_status` still aligns with one
    // that does, so two `show` outputs sit in the same column.
    let width = keys.iter().map(|k| k.chars().count()).max().unwrap_or(0);
    let shown: Vec<(String, String)> = keys.iter().filter_map(|k| field_value_raw(row, k).map(|v| (k.clone(), v))).collect();
    let mut out: Vec<String> = Vec::new();
    for (k, v) in &shown {
        let v = match k.as_str() {
            "created" | "started" | "closed" => v.get(..10).unwrap_or(v).to_string(),
            "id" => hl_id(v, Some(&abbrev), false),
            _ => v.clone(),
        };
        out.push(format!("{}  {v}", paint(&format!("{k:>width$}"), &["dim"])));
    }
    out.push(String::new());
    out.push("--- body ---".into());
    out.push(String::new());
    // Same wording as the mutating verbs' guard: a row whose body has gone missing is one
    // inconsistency, and it should read the same whichever verb runs into it. Passing the
    // raw io error through instead would name the file but not the issue.
    let path = issue_path(ctx, row);
    if !path.exists() {
        return Err(format!("file missing for #{}: {}", row.id, path.display()));
    }
    let body = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    out.push(body);
    Ok(out.join("\n"))
}
