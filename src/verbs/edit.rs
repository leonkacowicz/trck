//! The verbs that change an issue: create, move, edit, label, depend.
//!
//! They share a shape — load, resolve the id, mutate, guard, `finalize` — and the guards are
//! where the interest is. Everything that could leave the tracker inconsistent is checked
//! against the *candidate* state before anything is written, so a refusal leaves the files
//! exactly as they were.

use super::{TEMPLATE, apply_status, check_slug, finalize, issue_path, load_rows, now_utc, resolve_ref, slugify, write_atomic};
use crate::config::{self, is_terminal};
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::id;
use crate::issue::{DEFAULT_POINTS, Issue, check_field_key};
use crate::render::python_list;
use std::collections::BTreeMap;
use std::path::Path;

/// Options `new` accepts. A struct rather than a long parameter list, because the CLI
/// layer fills it field by field and a positional call would be unreadable.
#[derive(Default)]
pub(crate) struct NewOpts {
    pub(crate) title: String,
    pub(crate) id: Option<String>,
    pub(crate) slug: Option<String>,
    pub(crate) priority: Option<String>,
    pub(crate) points: Option<i64>,
    pub(crate) parent: Option<String>,
    pub(crate) depends: Vec<String>,
    pub(crate) spec: Option<String>,
    pub(crate) review_url: Option<String>,
}

pub(crate) fn cmd_new(ctx: &Ctx, opts: &NewOpts) -> Result<String, String> {
    let mut rows = load_rows(ctx)?;
    let taken = taken_ids(ctx, &rows);
    let iid = if let Some(v) = &opts.id {
        // A supplied id clears the same bar as a generated one: valid, and unused in
        // the index *and* on disk.
        if let Some(msg) = id::check(v) {
            return Err(msg);
        }
        if taken.contains(v) {
            return Err(format!("id '{v}' is already taken"));
        }
        v.clone()
    } else {
        id::generate(&|c| taken.contains(c))
    };
    let slug = opts.slug.clone().unwrap_or_else(|| slugify(&opts.title));
    if !check_slug(&slug) {
        return Err(format!("computed slug '{slug}' is invalid; pass --slug"));
    }
    let priority = opts.priority.clone().unwrap_or_else(|| config::default_priority().to_string());
    if let Some(msg) = config::check_priority(&priority) {
        return Err(msg);
    }
    let points = opts.points.unwrap_or(DEFAULT_POINTS);
    if let Some(msg) = config::check_points(points) {
        return Err(msg);
    }
    if let Some(url) = &opts.review_url
        && let Some(msg) = config::check_review_url(url)
    {
        return Err(msg);
    }
    let parent = opts.parent.as_ref().map(|p| resolve_ref(&rows, p)).transpose()?;
    let depends: Vec<String> = opts.depends.iter().map(|d| resolve_ref(&rows, d)).collect::<Result<_, _>>()?;

    let row = Issue {
        id: iid.clone(),
        slug,
        title: opts.title.clone(),
        status: config::initial_status().to_string(),
        priority,
        points,
        parent,
        labels: Vec::new(),
        depends_on: depends.clone(),
        spec: opts.spec.clone(),
        review_url: opts.review_url.clone(),
        created: Some(now_utc()?),
        started: None,
        closed: None,
        resolution: None,
        manual_status: false,
        extra: BTreeMap::new(),
    };
    let path = issue_path(ctx, &row);
    if path.exists() {
        return Err(format!("{} already exists", path.display()));
    }
    rows.push(row);

    // Guard the new node's edges against the candidate graph — its parent is already
    // set, so an inherited cousin cycle is caught — before writing anything.
    let g = Graph::new(rows);
    for dep in &depends {
        if let Some(msg) = g.check_dep_edge(&iid, dep) {
            return Err(msg);
        }
    }
    let rows = g.rows;
    write_atomic(&path, &TEMPLATE.replace("{title}", &opts.title))?;
    finalize(ctx, rows)?;
    Ok(path.display().to_string())
}

/// Every id visible: index rows plus on-disk filenames. A branch may carry a body file
/// whose index line has not merged yet, so checking only the index would let `--id`
/// reintroduce the collision random ids exist to prevent.
fn taken_ids(ctx: &Ctx, rows: &[Issue]) -> std::collections::BTreeSet<String> {
    let mut ids: std::collections::BTreeSet<String> = rows.iter().map(|r| r.id.clone()).collect();
    if let Ok(entries) = std::fs::read_dir(ctx.items_dir()) {
        for e in entries.flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if let Some(stem) = name.strip_suffix(".md")
                && let Some((id, _)) = stem.split_once('-')
            {
                ids.insert(id.to_string());
            }
        }
    }
    ids
}

pub(crate) fn cmd_mv(ctx: &Ctx, token: &str, status: &str, resolution: Option<&str>, review_url: Option<&str>) -> Result<String, String> {
    let mut rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    if let Some(res) = resolution {
        if !is_terminal(status) {
            return Err("--resolution is only valid when moving to a terminal status".into());
        }
        if let Some(msg) = config::check_resolution(res) {
            return Err(msg);
        }
    }
    if let Some(url) = review_url
        && let Some(msg) = config::check_review_url(url)
    {
        return Err(msg);
    }
    if let Some(msg) = config::check_status(status) {
        return Err(msg);
    }
    let path = {
        let g = Graph::new(std::mem::take(&mut rows));
        let p = g.get(&iid).map(|r| issue_path(ctx, r));
        rows = g.rows;
        p.ok_or_else(|| format!("no issue matching '{iid}'"))?
    };
    if !path.exists() {
        return Err(format!("file missing for #{iid}: {}", path.display()));
    }

    let kid_statuses: Vec<String> = {
        let g = Graph::new(std::mem::take(&mut rows));
        let ks = g.children_of(&iid).iter().filter_map(|k| g.get(k).map(|r| r.status.clone())).collect();
        rows = g.rows;
        ks
    };
    if let Some(row) = rows.iter_mut().find(|r| r.id == iid) {
        apply_status(row, status)?;
        if let Some(url) = review_url {
            row.review_url = Some(url.to_string());
        }
        if let Some(res) = resolution {
            row.resolution = Some(res.to_string());
        }
        // Moving a node with children overrides the rollup — but only when the
        // requested status differs from what derivation would produce. A move that
        // agrees with the children leaves it unpinned, so nothing to override.
        if !kid_statuses.is_empty() {
            row.manual_status = row.status != config::reconcile(&kid_statuses);
        }
    }
    finalize(ctx, rows)?;
    Ok(path.display().to_string())
}

/// Apply `--field key=value` and `--unset key`. An empty value clears, as an alias for
/// `--unset`, so `--field assignee=` reads the way people expect.
fn apply_field_edits(row: &mut Issue, fields: &[&str], unset: &[&str]) -> Result<(), String> {
    for spec in fields {
        let (key, val) = spec.split_once('=').ok_or_else(|| format!("--field expects key=value, got '{spec}'"))?;
        if let Some(msg) = check_field_key(key) {
            return Err(msg);
        }
        if val.is_empty() {
            row.extra.remove(key);
        } else {
            row.extra.insert(key.to_string(), crate::json::Json::String(val.to_string()));
        }
    }
    for key in unset {
        if let Some(msg) = check_field_key(key) {
            return Err(msg);
        }
        row.extra.remove(*key);
    }
    Ok(())
}

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

/// Apply every scalar edit `set` was asked for, validating each before it lands.
///
/// Split from `cmd_set` because it is the bulk of it and none of it is about orchestration:
/// each block is the same shape — was this option given, is the value legal, assign it — and
/// an early return leaves the row untouched from that point on, which is why nothing is
/// written until they have all passed.
fn apply_scalar_edits(row: &mut Issue, opts: &SetOpts, is_leaf: bool, parent: Option<String>) -> Result<(), String> {
    if opts.auto {
        row.manual_status = false; // back to derivation; finalize re-derives and cascades
    }
    if let Some(p) = opts.priority {
        if let Some(msg) = config::check_priority(p) {
            return Err(msg);
        }
        row.priority = p.to_string();
    }
    if let Some(points) = opts.points {
        if let Some(msg) = config::check_points(points) {
            return Err(msg);
        }
        if !is_leaf {
            return Err(format!("#{} has children; points is derived from them, not set", row.id));
        }
        row.points = points;
    }
    if opts.parent.is_some() {
        row.parent = parent;
    }
    if let Some(spec) = opts.spec {
        row.spec = (spec != "none").then(|| spec.to_string());
    }
    if let Some(url) = opts.review_url {
        if url != "none"
            && let Some(msg) = config::check_review_url(url)
        {
            return Err(msg);
        }
        row.review_url = (url != "none").then(|| url.to_string());
    }
    apply_field_edits(row, &opts.fields, &opts.unset)?;
    if let Some(slug) = opts.slug {
        if !check_slug(slug) {
            return Err(format!("invalid slug '{slug}'"));
        }
        row.slug = slug.to_string();
    }
    if let Some(title) = opts.title {
        row.title = title.to_string();
    }
    Ok(())
}

/// Refuse a re-parent that would close a dependency cycle, before anything is written.
///
/// Re-parenting changes what is lifted, so it can introduce an effective cycle that neither
/// authored edge shows. The message is composed while the graph is still alive: the node
/// loop alone says a cycle exists but not which edge to remove, and a re-parent has no
/// single edge to point at — which is exactly why it has to be spelled out.
fn guard_reparent(rows: Vec<Issue>) -> (Vec<Issue>, Option<String>) {
    let g = Graph::new(rows);
    let refusal = g.parent_cycles().first().map_or_else(
        || {
            g.effective_cycles()
                .first()
                .map(|cyc| format!("this change would create an effective dependency cycle: {}", crate::validate::describe_cycle(&g, cyc)))
        },
        |cyc| Some(format!("parent cycle: {}", cyc.join(" -> "))),
    );
    (g.rows, refusal)
}

pub(crate) fn cmd_set(ctx: &Ctx, token: &str, opts: &SetOpts) -> Result<String, String> {
    let mut rows = load_rows(ctx)?;
    let iid = resolve_ref(&rows, token)?;
    let g = Graph::new(std::mem::take(&mut rows));
    let is_leaf = g.is_leaf(&iid);
    let parent = opts.parent.filter(|p| *p != "none").map(|p| resolve_ref(&g.rows, p)).transpose()?;
    rows = g.rows;

    let missing = || format!("no issue matching '{iid}'");
    let old_path = rows.iter().find(|r| r.id == iid).map(|r| issue_path(ctx, r)).ok_or_else(missing)?;
    let row = rows.iter_mut().find(|r| r.id == iid).ok_or_else(missing)?;
    apply_scalar_edits(row, opts, is_leaf, parent)?;
    let new_path = rows.iter().find(|r| r.id == iid).map_or_else(|| old_path.clone(), |r| issue_path(ctx, r));

    if opts.parent.is_some() {
        let (returned, refusal) = guard_reparent(std::mem::take(&mut rows));
        rows = returned;
        if let Some(msg) = refusal {
            return Err(msg);
        }
    }

    if old_path != new_path {
        std::fs::rename(&old_path, &new_path).map_err(|e| format!("{} -> {}: {e}", old_path.display(), new_path.display()))?;
    }
    if let Some(title) = opts.title {
        retitle_body(&new_path, title)?;
    }
    finalize(ctx, rows)?;
    Ok(format!("#{iid} updated"))
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
    if let Some(target) = &add {
        let g = Graph::new(std::mem::take(&mut rows));
        let refusal = g.check_dep_edge(&iid, target);
        rows = g.rows;
        if let Some(msg) = refusal {
            return Err(msg);
        }
    }
    let Some(row) = rows.iter_mut().find(|r| r.id == iid) else {
        return Err(format!("no issue matching '{iid}'"));
    };
    if let Some(target) = add
        && !row.depends_on.contains(&target)
    {
        row.depends_on.push(target);
    }
    if let Some(target) = remove {
        row.depends_on.retain(|d| *d != target);
    }
    row.depends_on.sort();
    let shown = python_list(&row.depends_on);
    finalize(ctx, rows)?;
    Ok(format!("#{iid} depends_on={shown}"))
}
