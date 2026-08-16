//! `list` and `tree`: rendering the selected rows as a forest, a flat list, JSON, or paths.
//!
//! Separated from the other read verbs because it is the only one that filters. What to
//! *keep* and how to *show* it are different questions, and the first of them is
//! [`super::filter`] — this file is the second.

use super::filter::{RowFilter, body_hits};
use super::{ListOpts, rank};
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::issue::Issue;
use crate::json::Json;
use crate::render::{Annotation, RowOpts, render_rows, unique_prefix_lens};
use crate::verbs::{load_rows, resolve_ref};
use std::collections::{BTreeMap, BTreeSet};

/// One nested node of `list --json`, recursing into the children that are on screen.
///
/// A node carries two keys the flat form has no use for: `children`, so a consumer wanting
/// the hierarchy need not rebuild it from `parent` pointers, and `context`, marking a row
/// present only to keep a match attached to its ancestors — what the human view dims.
/// `seen` guards a hand-edited parent cycle the same way the human forest walk does.
fn json_node(g: &Graph, sel: &Selection, id: &str, sorted: &mut impl FnMut(&mut Vec<String>), seen: &mut BTreeSet<String>) -> Json {
    let mut kids: Vec<String> = g.children_of(id).iter().filter(|c| sel.shown.contains(*c)).cloned().collect();
    sorted(&mut kids);
    let children: Vec<Json> = if seen.insert(id.to_string()) { kids.iter().map(|c| json_node(g, sel, c, sorted, seen)).collect() } else { Vec::new() };
    let mut obj = match g.get(id).map(Issue::to_full) {
        Some(Json::Object(pairs)) => pairs,
        _ => Vec::new(),
    };
    obj.push(("context".into(), Json::Bool(!sel.matched.contains(id))));
    obj.push(("children".into(), Json::Array(children)));
    Json::Object(obj)
}

pub(crate) fn cmd_list(ctx: &Ctx, opts: &ListOpts) -> Result<String, String> {
    let rows = load_rows(ctx)?;
    let root = opts.root.map(|t| resolve_ref(&rows, t)).transpose()?;
    let parent_filter = opts.parent.map(|t| resolve_ref(&rows, t)).transpose()?;
    // Before the graph, because it is a search over the tracker's bodies rather than a
    // question about a row, and it runs exactly once however many rows there turn out to be.
    let contains = body_hits(ctx, &rows, opts.contains)?;
    let g = Graph::new(rows);

    let filter = RowFilter::build(opts, parent_filter, contains)?;
    let keep = |r: &Issue| filter.keeps(&g, r);

    let sort = opts.sort.unwrap_or("created");
    let rank = rank::sibling_rank(&g, |id| rank::seed_key(&g, id, sort));
    let mut sorted = |ids: &mut Vec<String>| {
        ids.sort_by_cached_key(|id| (rank.get(id).copied().unwrap_or(usize::MAX), rank::seed_key(&g, id, sort)));
    };

    if opts.paths {
        return super::paths::paths_of(ctx, &g, &selected(&g, &keep, &mut sorted));
    }
    if opts.json {
        return Ok(list_json(&g, opts, &keep, &mut sorted, root.as_deref()));
    }

    let abbrev = unique_prefix_lens(g.rows.iter().map(|r| r.id.as_str()));
    let show_fields: Vec<String> = opts.show_fields.iter().map(|s| (*s).to_string()).collect();
    if opts.flat {
        return Ok(flat_rows(&g, &selected(&g, &keep, &mut sorted), show_fields, abbrev));
    }
    Ok(forest(&g, opts, &keep, &mut sorted, show_fields, abbrev, root.as_deref()))
}

/// The kept ids, in the requested order — the input to both flat output modes.
fn selected(g: &Graph, keep: &impl Fn(&Issue) -> bool, sorted: &mut impl FnMut(&mut Vec<String>)) -> Vec<String> {
    let mut ids: Vec<String> = g.rows.iter().filter(|r| keep(r)).map(|r| r.id.clone()).collect();
    sorted(&mut ids);
    ids
}

/// `--flat`: one row per issue, globally sorted, with no tree structure to carry.
fn flat_rows(g: &Graph, ids: &[String], show_fields: Vec<String>, abbrev: BTreeMap<String, usize>) -> String {
    let rows: Vec<&Issue> = ids.iter().filter_map(|id| g.get(id)).collect();
    let row_opts =
        RowOpts { prefix: None, dim: &[], on_screen: ids.to_vec(), annotate: Annotation::Blocking, progress: true, show_fields, abbrev: Some(abbrev) };
    render_rows(g, &rows, &row_opts).join("\n")
}

/// Which rows the nested view covers: the matches, everything shown (matches plus the
/// ancestor spine that keeps them attached), and the roots to walk from.
struct Selection {
    matched: BTreeSet<String>,
    shown: BTreeSet<String>,
    roots: Vec<String>,
}

/// The nested view's row selection, shared by the human forest and the `--json` one so the
/// two can never disagree about what a filter selected — only about how it is rendered.
fn select_forest(g: &Graph, keep: &impl Fn(&Issue) -> bool, sorted: &mut impl FnMut(&mut Vec<String>), root: Option<&str>) -> Selection {
    let matched: BTreeSet<String> = g.rows.iter().filter(|r| keep(r)).map(|r| r.id.clone()).collect();
    let mut shown = matched.clone();
    for id in &matched {
        shown.extend(g.ancestors_of(id));
    }
    let mut roots: Vec<String> = if let Some(id) = root {
        if shown.contains(id) { vec![id.to_string()] } else { Vec::new() }
    } else {
        g.rows.iter().filter(|r| shown.contains(&r.id) && r.parent.as_ref().is_none_or(|p| g.get(p).is_none())).map(|r| r.id.clone()).collect()
    };
    sorted(&mut roots);
    Selection { matched, shown, roots }
}

/// `list --json`: the rows the human view would print, as one document.
///
/// Takes the *same* `keep` and `sorted` the human path built, so the two renderings cannot
/// drift on what a filter selected — only on how it is shown. Nested by default and flat
/// under `--flat`, mirroring the human view: a consumer wanting the hierarchy should not
/// have to rebuild it from `parent` pointers. Rows are [`Issue::to_full`], so every
/// canonical key is present even where unset.
fn list_json(g: &Graph, opts: &ListOpts, keep: &impl Fn(&Issue) -> bool, sorted: &mut impl FnMut(&mut Vec<String>), root: Option<&str>) -> String {
    if opts.flat {
        let mut ids: Vec<String> = g.rows.iter().filter(|r| keep(r)).map(|r| r.id.clone()).collect();
        sorted(&mut ids);
        let rows: Vec<Json> = ids.iter().filter_map(|id| g.get(id)).map(Issue::to_full).collect();
        return Json::Array(rows).to_json_pretty();
    }
    let sel = select_forest(g, keep, sorted, root);
    let nodes: Vec<Json> = sel.roots.iter().map(|id| json_node(g, &sel, id, sorted, &mut BTreeSet::new())).collect();
    Json::Array(nodes).to_json_pretty()
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

    let Selection { matched, shown, roots } = select_forest(g, keep, sorted, root);
    let dim: Vec<String> = shown.difference(&matched).cloned().collect();

    let mut f = Forest { shown: &shown, ordered: Vec::new(), prefixes: BTreeMap::new(), seen: BTreeSet::new() };
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
    let mut kids: Vec<String> = g.children_of(id).iter().filter(|k| f.shown.contains(*k)).cloned().collect();
    sorted(&mut kids);
    let last_index = kids.len().saturating_sub(1);
    for (i, kid) in kids.iter().enumerate() {
        let last = i == last_index;
        f.ordered.push(kid.clone());
        f.prefixes.insert(kid.clone(), format!("{pfx}{}", if last { "└─ " } else { "├─ " }));
        if f.seen.insert(kid.clone()) {
            let ext = format!("{pfx}{}", if last { "   " } else { "│  " });
            walk(g, f, kid, &ext, sorted);
        }
    }
}
