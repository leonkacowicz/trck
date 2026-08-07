//! `new`: the only verb that invents a row rather than editing one.
//!
//! Everything it derives — id, slug, priority, points — has a default and a rule the tracker
//! would enforce later anyway, so each is checked here, before the row exists to hold it.

use super::super::{TEMPLATE, check_slug, finalize, issue_path, load_rows, now_utc, resolve_ref, slugify, write_atomic};
use crate::config;
use crate::discovery::Ctx;
use crate::graph::Graph;
use crate::id;
use crate::issue::{DEFAULT_POINTS, Issue};
use std::collections::{BTreeMap, BTreeSet};

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
    let row = build_row(ctx, &rows, opts)?;
    let (iid, depends) = (row.id.clone(), row.depends_on.clone());
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

/// The candidate row: every field defaulted, resolved and validated, but nothing written.
///
/// Its edges are *not* checked here — that needs the graph this row is about to join, which
/// only exists once the row does.
fn build_row(ctx: &Ctx, rows: &[Issue], opts: &NewOpts) -> Result<Issue, String> {
    // Left-to-right in the order the fields are given, so which complaint comes back does
    // not depend on which check happens to be cheap.
    let id = new_id(ctx, rows, opts.id.as_deref())?;
    let slug = checked_slug(opts)?;
    let priority = checked_priority(opts)?;
    let points = checked_points(opts)?;
    if let Some(url) = &opts.review_url
        && let Some(msg) = config::check_review_url(url)
    {
        return Err(msg);
    }
    let (parent, depends_on) = resolve_edges(rows, opts)?;
    Ok(Issue {
        id,
        slug,
        title: opts.title.clone(),
        status: config::BACKLOG.to_string(),
        priority,
        points,
        parent,
        labels: Vec::new(),
        depends_on,
        spec: opts.spec.clone(),
        review_url: opts.review_url.clone(),
        created: Some(now_utc()?),
        started: None,
        closed: None,
        resolution: None,
        manual_status: false,
        extra: BTreeMap::new(),
    })
}

/// `--parent` and `--requires`, resolved from prefixes to full ids against the tracker as
/// it stands. Whether the resulting edges are *legal* is a question for the candidate graph,
/// which does not exist yet.
fn resolve_edges(rows: &[Issue], opts: &NewOpts) -> Result<(Option<String>, Vec<String>), String> {
    let parent = opts.parent.as_ref().map(|p| resolve_ref(rows, p)).transpose()?;
    let depends = opts.depends.iter().map(|d| resolve_ref(rows, d)).collect::<Result<_, _>>()?;
    Ok((parent, depends))
}

/// The id for the new issue: a fresh one, or the one that was asked for once it clears the
/// same bar a generated one does — valid, and unused in the index *and* on disk.
fn new_id(ctx: &Ctx, rows: &[Issue], want: Option<&str>) -> Result<String, String> {
    let taken = taken_ids(ctx, rows);
    let Some(v) = want else {
        return Ok(id::generate(&|c| taken.contains(c)));
    };
    if let Some(msg) = id::check(v) {
        return Err(msg);
    }
    if taken.contains(v) {
        return Err(format!("id '{v}' is already taken"));
    }
    Ok(v.to_string())
}

/// Every id visible: index rows plus on-disk filenames. A branch may carry a body file
/// whose index line has not merged yet, so checking only the index would let `--id`
/// reintroduce the collision random ids exist to prevent.
fn taken_ids(ctx: &Ctx, rows: &[Issue]) -> BTreeSet<String> {
    let mut ids: BTreeSet<String> = rows.iter().map(|r| r.id.clone()).collect();
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

/// The slug: given, or derived from the title. A derived one can still be unusable — a title
/// of nothing but punctuation slugifies to the empty string — hence the check either way.
fn checked_slug(opts: &NewOpts) -> Result<String, String> {
    let slug = opts.slug.clone().unwrap_or_else(|| slugify(&opts.title));
    if check_slug(&slug) { Ok(slug) } else { Err(format!("computed slug '{slug}' is invalid; pass --slug")) }
}

fn checked_priority(opts: &NewOpts) -> Result<String, String> {
    let priority = opts.priority.clone().unwrap_or_else(|| config::default_priority().to_string());
    config::check_priority(&priority).map_or(Ok(priority), Err)
}

fn checked_points(opts: &NewOpts) -> Result<i64, String> {
    let points = opts.points.unwrap_or(DEFAULT_POINTS);
    config::check_points(points).map_or(Ok(points), Err)
}
