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

use crate::config::is_terminal;
use crate::discovery::Ctx;
use crate::graph::{Graph, priority_rank};
use crate::gutter;
use crate::issue::{CANON_KEYS, Issue, check_field_key};
use crate::render::{
    Annotation, LANE_PALETTE, RowOpts, field_value, hl_id, lane_palette_index, paint, render_rows,
    status_codes, status_icon, unique_prefix_lens,
};
use crate::verbs::{issue_path, load_rows, resolve_ref};
use std::collections::{BTreeMap, BTreeSet};

/// Everything `list` accepts.
///
/// Five booleans, which clippy dislikes and which is right anyway: they mirror the CLI
/// flags one-to-one, and folding them into an enum would hide that `--flat --paths` is a
/// combination the caller can express and this code has to answer for.
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors the CLI flags one-to-one"
)]
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
}

/// `--status a,b` keeps those; `--status '!done'` drops them. Returns `(keep, drop)`.
fn parse_status_filter(spec: Option<&str>) -> (BTreeSet<String>, BTreeSet<String>) {
    let (mut keep, mut drop) = (BTreeSet::new(), BTreeSet::new());
    for part in spec.unwrap_or("").split(',').map(str::trim) {
        if part.is_empty() {
            continue;
        }
        if let Some(name) = part.strip_prefix('!') {
            drop.insert(name.to_string());
        } else {
            keep.insert(part.to_string());
        }
    }
    (keep, drop)
}

/// The sort key for a row, as a tuple that compares in the right order.
fn sort_key(g: &Graph, r: &Issue, sort: &str) -> (usize, String, String) {
    match sort {
        "priority" => (priority_rank(&r.priority), String::new(), r.id.clone()),
        "points" => (
            // Descending, so a bigger weight sorts first.
            usize::MAX - usize::try_from(r.points.max(0)).unwrap_or(0),
            String::new(),
            r.id.clone(),
        ),
        "id" => (0, r.id.clone(), r.id.clone()),
        _ if sort.starts_with("field:") => {
            let name = &sort["field:".len()..];
            // Rows carrying the field sort by value; rows without it sort last.
            field_value(r, name).map_or_else(
                || (1, String::new(), r.id.clone()),
                |v| (0, v, r.id.clone()),
            )
        }
        _ => {
            let _ = g;
            (0, r.created.clone().unwrap_or_default(), r.id.clone())
        }
    }
}

/// Validate the option combination before any work: an unknown `--sort` or a malformed
/// `--field` should be reported as such, not silently ignored.
fn check_list_opts(opts: &ListOpts) -> Result<Vec<(String, String)>, String> {
    let mut field_filters = Vec::new();
    for spec in &opts.fields {
        let (k, v) = spec
            .split_once('=')
            .ok_or_else(|| format!("--field expects key=value, got '{spec}'"))?;
        // The same key rule the write side enforces. Without it a filter on a built-in —
        // `--field status=backlog` — would look up a custom field that can never exist and
        // quietly match nothing, or worse, appear to work.
        if let Some(msg) = check_field_key(k) {
            return Err(msg);
        }
        field_filters.push((k.to_string(), v.to_string()));
    }
    if let Some(s) = opts.sort
        && !["priority", "points", "created", "id"].contains(&s)
        && !s.starts_with("field:")
    {
        return Err(format!(
            "unknown --sort '{s}' (choices: id, priority, points, created, field:NAME)"
        ));
    }
    Ok(field_filters)
}

pub(crate) fn cmd_list(ctx: &Ctx, opts: &ListOpts) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let root = opts.root.map(|t| resolve_ref(&rows, t)).transpose()?;
    let parent_filter = opts.parent.map(|t| resolve_ref(&rows, t)).transpose()?;
    let g = Graph::new(rows);

    let (keep_st, drop_st) = parse_status_filter(opts.status);
    let want = opts.match_title.unwrap_or("").to_lowercase();
    let field_filters = check_list_opts(opts)?;

    // The default view hides settled work. An explicit --status or --all bypasses it.
    let prune_settled = opts.status.is_none() && !opts.all;
    let settled = |r: &Issue| {
        if !is_terminal(&r.status) {
            return false;
        }
        r.parent
            .as_ref()
            .and_then(|p| g.get(p))
            .is_none_or(|p| is_terminal(&p.status))
    };

    let keep = |r: &Issue| {
        (keep_st.is_empty() || keep_st.contains(&r.status))
            && !drop_st.contains(&r.status)
            && opts.priority.is_none_or(|p| r.priority == p)
            && opts.label.is_none_or(|l| r.labels.iter().any(|x| x == l))
            && parent_filter
                .as_ref()
                .is_none_or(|p| r.parent.as_ref() == Some(p))
            && (want.is_empty() || r.title.to_lowercase().contains(&want))
            && (!opts.blocked || g.is_blocked(&r.id))
            && (!opts.orphan || r.parent.is_none())
            && (!prune_settled || !settled(r))
            && field_filters
                .iter()
                .all(|(k, v)| field_value(r, k).as_ref() == Some(v))
    };

    let sort = opts.sort.unwrap_or("created");
    let mut sorted = |ids: &mut Vec<String>| {
        ids.sort_by_cached_key(|id| {
            g.get(id)
                .map_or_else(|| (0, String::new(), id.clone()), |r| sort_key(&g, r, sort))
        });
    };

    if opts.paths {
        let mut ids: Vec<String> = g
            .rows
            .iter()
            .filter(|r| keep(r))
            .map(|r| r.id.clone())
            .collect();
        sorted(&mut ids);
        return Ok(paths_of(ctx, &g, &ids));
    }

    let abbrev = unique_prefix_lens(g.rows.iter().map(|r| r.id.as_str()));
    let show_fields: Vec<String> = opts.show_fields.iter().map(|s| (*s).to_string()).collect();

    if opts.flat {
        let mut ids: Vec<String> = g
            .rows
            .iter()
            .filter(|r| keep(r))
            .map(|r| r.id.clone())
            .collect();
        sorted(&mut ids);
        let rows: Vec<&Issue> = ids.iter().filter_map(|id| g.get(id)).collect();
        let row_opts = RowOpts {
            prefix: None,
            dim: &[],
            on_screen: ids.clone(),
            annotate: Annotation::Blocking,
            progress: true,
            show_fields,
            abbrev: Some(abbrev),
        };
        return Ok(render_rows(&g, &rows, &row_opts).join("\n"));
    }
    Ok(forest(
        &g,
        opts,
        &keep,
        &mut sorted,
        show_fields,
        abbrev,
        root.as_deref(),
    ))
}

/// The nested view: show a node iff it matches or has a matching descendant, with the
/// non-matching ancestors kept as dimmed context so a matched child never floats free.
fn forest(
    g: &Graph,
    opts: &ListOpts,
    keep: &impl Fn(&Issue) -> bool,
    sorted: &mut impl FnMut(&mut Vec<String>),
    show_fields: Vec<String>,
    abbrev: BTreeMap<String, usize>,
    root: Option<&str>,
) -> String {
    let _ = opts;

    let matched: BTreeSet<String> = g
        .rows
        .iter()
        .filter(|r| keep(r))
        .map(|r| r.id.clone())
        .collect();
    let mut shown = matched.clone();
    for id in &matched {
        shown.extend(g.ancestors_of(id));
    }
    let dim: Vec<String> = shown.difference(&matched).cloned().collect();

    let mut roots: Vec<String> = if let Some(id) = root {
        if shown.contains(id) {
            vec![id.to_string()]
        } else {
            Vec::new()
        }
    } else {
        g.rows
            .iter()
            .filter(|r| {
                shown.contains(&r.id) && r.parent.as_ref().is_none_or(|p| g.get(p).is_none())
            })
            .map(|r| r.id.clone())
            .collect()
    };
    sorted(&mut roots);

    let mut f = Forest {
        shown: &shown,
        ordered: Vec::new(),
        prefixes: BTreeMap::new(),
        seen: BTreeSet::new(),
    };
    for r in &roots {
        f.ordered.push(r.clone());
        f.prefixes.insert(r.clone(), String::new());
        f.seen = BTreeSet::from([r.clone()]);
        walk(g, &mut f, r, "", sorted);
    }

    let rows: Vec<&Issue> = f.ordered.iter().filter_map(|id| g.get(id)).collect();
    let row_opts = RowOpts {
        prefix: Some(&f.prefixes),
        dim: &dim,
        on_screen: f.ordered.clone(),
        annotate: Annotation::Blocking,
        progress: true,
        show_fields,
        abbrev: Some(abbrev),
    };
    render_rows(g, &rows, &row_opts).join("\n")
}

/// Absolute body paths, one per line — what `--paths` prints for piping into an editor.
fn paths_of(ctx: &Ctx, g: &Graph, ids: &[String]) -> String {
    ids.iter()
        .filter_map(|id| g.get(id))
        .map(|r| {
            let p = issue_path(ctx, r);
            p.canonicalize().unwrap_or(p).display().to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// What the forest walk accumulates. A struct rather than eight parameters, which is
/// also the honest shape: these five move together.
struct Forest<'a> {
    shown: &'a BTreeSet<String>,
    ordered: Vec<String>,
    prefixes: BTreeMap<String, String>,
    seen: BTreeSet<String>,
}

/// Depth-first walk building the connector prefixes, cycle-guarded.
fn walk(g: &Graph, f: &mut Forest, id: &str, pfx: &str, sorted: &mut impl FnMut(&mut Vec<String>)) {
    let mut kids: Vec<String> = g
        .children_of(id)
        .iter()
        .filter(|k| f.shown.contains(*k))
        .cloned()
        .collect();
    sorted(&mut kids);
    let last_index = kids.len().saturating_sub(1);
    for (i, kid) in kids.iter().enumerate() {
        let last = i == last_index;
        f.ordered.push(kid.clone());
        f.prefixes.insert(
            kid.clone(),
            format!("{pfx}{}", if last { "└─ " } else { "├─ " }),
        );
        if f.seen.insert(kid.clone()) {
            let ext = format!("{pfx}{}", if last { "   " } else { "│  " });
            walk(g, f, kid, &ext, sorted);
        }
    }
}

/// `ready` lists the unblocked leaves in rank order; `next` is the same, capped at one.
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
#[allow(
    clippy::struct_excessive_bools,
    reason = "mirrors the CLI flags one-to-one"
)]
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
            return Ok(format!(
                "{}  (no dependencies)",
                node_label(&g, row, true, Some(&abbrev))
            ));
        }
        if opts.full {
            // The focal node's whole component, computed over *every* issue — not over
            // the overview set, which drops the components the bare view suppresses and
            // could therefore lose the focal node itself.
            let all: BTreeSet<String> = g.rows.iter().map(|r| r.id.clone()).collect();
            let edges = gutter::drawn_edges(&g, &all, false, false);
            gutter::components(&all, &edges)
                .into_iter()
                .find(|c| c.contains(id))
                .unwrap_or_default()
                .into_iter()
                .collect()
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
    let ids = gutter::filter_done(
        &g,
        &ids,
        opts.omit_done,
        opts.include_done_chains,
        root.is_none(),
    );

    let rendered = gutter::render_graph(&g, &ids, opts.fanout);
    let width = rendered
        .iter()
        .flatten()
        .map(|(_, gut, _)| gut.chars().count())
        .max()
        .unwrap_or(0);
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
        out.push(format!(
            "{marker}{painted}{}  {}",
            " ".repeat(width - gut.chars().count()),
            node_label(&g, row, focal, Some(&abbrev))
        ));
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
                }
            }
        })
        .collect()
}

/// One node's label in the graph: id, status icon, title, and a derived epic marker.
///
/// `·epic·` comes from the hierarchy, not from a stored kind — an issue with children
/// *is* an epic, and a declared marker only drifts from that.
fn node_label(
    g: &Graph,
    r: &Issue,
    focal: bool,
    abbrev: Option<&BTreeMap<String, usize>>,
) -> String {
    let tag = if g.children_of(&r.id).is_empty() {
        String::new()
    } else {
        " ·epic·".to_string()
    };
    let labels = if r.labels.is_empty() {
        String::new()
    } else {
        paint(&format!(" [{}]", r.labels.join(" ")), &["dim"])
    };
    let emph: &[&str] = if focal { &["bold"] } else { &[] };
    format!(
        "{} {} {}{tag}{labels}",
        hl_id(&r.id, abbrev, true),
        paint(status_icon(&r.status), &status_codes(&r.status)),
        paint(&r.title, emph)
    )
}

pub(crate) fn cmd_show(ctx: &Ctx, token: &str) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    let abbrev = unique_prefix_lens(rows.iter().map(|r| r.id.as_str()));
    let g = Graph::new(rows);
    let row = g
        .get(&iid)
        .ok_or_else(|| format!("no issue matching '{iid}'"))?;

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
    let shown: Vec<(String, String)> = keys
        .iter()
        .filter_map(|k| field_value(row, k).map(|v| (k.clone(), v)))
        .collect();
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
    let body = std::fs::read_to_string(issue_path(ctx, row))
        .map_err(|e| format!("{}: {e}", issue_path(ctx, row).display()))?;
    out.push(body);
    Ok(out.join("\n"))
}

#[cfg(test)]
mod tests {
    // Tests assert; that is their job. The crate denies unwrap/expect/panic because a
    // malformed tracker must produce a diagnostic rather than a stack trace, but a test
    // that cannot panic cannot fail.
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use super::*;

    #[test]
    fn a_status_filter_separates_keeps_from_drops() {
        let (keep, drop) = parse_status_filter(Some("backlog,ongoing"));
        assert_eq!(keep.len(), 2);
        assert!(drop.is_empty());
        let (keep, drop) = parse_status_filter(Some("!done"));
        assert!(keep.is_empty());
        assert!(drop.contains("done"));
    }

    #[test]
    fn an_absent_filter_keeps_everything() {
        let (keep, drop) = parse_status_filter(None);
        assert!(keep.is_empty() && drop.is_empty());
    }
}
