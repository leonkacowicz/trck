//! `set`: the verb that edits an existing issue's metadata.
//!
//! Every edit is validated before *any* of it lands. That split — a fallible check pass over
//! the row, then an infallible apply — is what keeps a half-edited row out of reach; the two
//! used to be one pass, safe only because nothing was persisted until it returned, which is a
//! much weaker guarantee than not being able to fail.

use super::super::{check_slug, finalize, issue_path, load_rows, resolve_ref, write_atomic};
use crate::config;
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::issue::{Issue, check_field_key};
use std::path::Path;

/// Options `set` accepts. `Option<&str>` throughout, because "not given" and "given as
/// `none`" mean different things: the first leaves a field alone, the second clears it.
#[derive(Default)]
pub(crate) struct SetOpts<'a> {
    pub(crate) auto: bool,
    pub(crate) priority: Option<&'a str>,
    pub(crate) points: Option<i64>,
    pub(crate) parent: Option<&'a str>,
    pub(crate) spec: Option<&'a str>,
    pub(crate) review_url: Option<&'a str>,
    pub(crate) title: Option<&'a str>,
    pub(crate) slug: Option<&'a str>,
    pub(crate) fields: Vec<&'a str>,
    pub(crate) unset: Vec<&'a str>,
}

pub(crate) fn cmd_set(ctx: &Ctx, token: &str, opts: &SetOpts) -> Result<String, String> {
    let mut rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    let (is_leaf, parent) = graph_context(&mut rows, &iid, opts.parent)?;

    let missing = || format!("no issue matching '{iid}'");
    let old_path = body_path(ctx, &rows, &iid).ok_or_else(missing)?;
    let row = rows.iter_mut().find(|r| r.id == iid).ok_or_else(missing)?;
    check_scalar_edits(row, opts, is_leaf)?;
    apply_scalar_edits(row, opts, parent);
    let new_path = body_path(ctx, &rows, &iid).unwrap_or_else(|| old_path.clone());

    if opts.parent.is_some() {
        guard_reparent(&mut rows)?;
    }
    follow_row(&old_path, &new_path, opts.title)?;
    finalize(ctx, rows)?;
    Ok(format!("#{iid} updated"))
}

/// The two things `set` can only learn from the graph: whether the row is a leaf — points
/// are derived on anything else — and what `--parent` resolves to. Both are read while the
/// graph owns the rows, which is why they come back together.
fn graph_context(rows: &mut Vec<Issue>, iid: &str, parent: Option<&str>) -> Result<(bool, Option<String>), String> {
    let g = Graph::new(std::mem::take(rows));
    let is_leaf = g.is_leaf(iid);
    let resolved = parent.filter(|p| *p != "none").map(|p| resolve_ref(&g.rows, p)).transpose();
    *rows = g.rows;
    Ok((is_leaf, resolved?))
}

/// Where the row's body file lives — a function of its slug, and so something `set` can
/// move out from under itself.
fn body_path(ctx: &Ctx, rows: &[Issue], iid: &str) -> Option<std::path::PathBuf> {
    rows.iter().find(|r| r.id == iid).map(|r| issue_path(ctx, r))
}

/// Refuse every illegal edit before the first legal one lands, in the order `set` applies
/// them — so which complaint comes back does not depend on which edit happens to be cheap
/// to check.
fn check_scalar_edits(row: &Issue, opts: &SetOpts, is_leaf: bool) -> Result<(), String> {
    if let Some(p) = opts.priority
        && let Some(msg) = config::check_priority(p)
    {
        return Err(msg);
    }
    if let Some(points) = opts.points {
        if let Some(msg) = config::check_points(points) {
            return Err(msg);
        }
        if !is_leaf {
            return Err(format!("#{} has children; points is derived from them, not set", row.id));
        }
    }
    if let Some(url) = opts.review_url
        && url != "none"
        && let Some(msg) = config::check_review_url(url)
    {
        return Err(msg);
    }
    check_field_edits(&opts.fields, &opts.unset)?;
    if let Some(slug) = opts.slug
        && !check_slug(slug)
    {
        return Err(format!("invalid slug '{slug}'"));
    }
    Ok(())
}

/// Apply every scalar edit `set` was asked for. Infallible by construction: anything that
/// could have been refused was refused by `check_scalar_edits`.
fn apply_scalar_edits(row: &mut Issue, opts: &SetOpts, parent: Option<String>) {
    if opts.auto {
        row.manual_status = false; // back to derivation; finalize re-derives and cascades
    }
    if let Some(p) = opts.priority {
        row.priority = p.to_string();
    }
    if let Some(points) = opts.points {
        row.points = points;
    }
    if opts.parent.is_some() {
        row.parent = parent;
    }
    if let Some(spec) = opts.spec {
        row.spec = (spec != "none").then(|| spec.to_string());
    }
    if let Some(url) = opts.review_url {
        row.review_url = (url != "none").then(|| url.to_string());
    }
    apply_field_edits(row, &opts.fields, &opts.unset);
    if let Some(slug) = opts.slug {
        row.slug = slug.to_string();
    }
    if let Some(title) = opts.title {
        row.title = title.to_string();
    }
}

/// One `--field key=value`, split and key-checked. The apply pass repeats this parse rather
/// than carrying its result along, which is what lets that pass be infallible.
fn parse_field(spec: &str) -> Result<(&str, &str), String> {
    let (key, val) = spec.split_once('=').ok_or_else(|| format!("--field expects key=value, got '{spec}'"))?;
    match check_field_key(key) {
        Some(msg) => Err(msg),
        None => Ok((key, val)),
    }
}

fn check_field_edits(fields: &[&str], unset: &[&str]) -> Result<(), String> {
    for spec in fields {
        parse_field(spec)?;
    }
    for key in unset {
        if let Some(msg) = check_field_key(key) {
            return Err(msg);
        }
    }
    Ok(())
}

/// Apply `--field key=value` and `--unset key`. An empty value clears, as an alias for
/// `--unset`, so `--field assignee=` reads the way people expect. A spec that did not parse
/// was already refused, so it is skipped here rather than reported twice.
fn apply_field_edits(row: &mut Issue, fields: &[&str], unset: &[&str]) {
    for (key, val) in fields.iter().filter_map(|s| parse_field(s).ok()) {
        if val.is_empty() {
            row.extra.remove(key);
        } else {
            row.extra.insert(key.to_string(), crate::json::Json::String(val.to_string()));
        }
    }
    for key in unset {
        row.extra.remove(*key);
    }
}

/// Refuse a re-parent that would close a dependency cycle, before anything is written.
///
/// Re-parenting changes what is lifted, so it can introduce an effective cycle that neither
/// authored edge shows. The message is composed while the graph is still alive: the node
/// loop alone says a cycle exists but not which edge to remove, and a re-parent has no
/// single edge to point at — which is exactly why it has to be spelled out.
fn guard_reparent(rows: &mut Vec<Issue>) -> Result<(), String> {
    let g = Graph::new(std::mem::take(rows));
    let refusal = g.parent_cycles().first().map_or_else(
        || {
            g.effective_cycles()
                .first()
                .map(|cyc| format!("this change would create an effective dependency cycle: {}", crate::validate::describe_cycle(&g, cyc)))
        },
        |cyc| Some(format!("parent cycle: {}", cyc.join(" -> "))),
    );
    *rows = g.rows;
    refusal.map_or(Ok(()), Err)
}

/// Bring the body file back in line with the row it belongs to: renamed when the slug moved,
/// re-headed when the title did.
fn follow_row(old: &Path, new: &Path, title: Option<&str>) -> Result<(), String> {
    if old != new {
        std::fs::rename(old, new).map_err(|e| format!("{} -> {}: {e}", old.display(), new.display()))?;
    }
    match title {
        Some(title) => retitle_body(new, title),
        None => Ok(()),
    }
}

/// Rewrite the body's first heading, so the file does not contradict the index. Only
/// the first line, and only when it is a heading — the rest is hand-authored prose.
fn retitle_body(path: &Path, title: &str) -> Result<(), String> {
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(()); // a missing body is `check`'s business, not this verb's
    };
    let rewritten: Vec<String> =
        text.lines().enumerate().map(|(i, line)| if i == 0 && line.starts_with("# ") { format!("# {title}") } else { line.to_string() }).collect();
    let mut body = rewritten.join("\n");
    if text.ends_with('\n') {
        body.push('\n');
    }
    write_atomic(path, &body)
}
